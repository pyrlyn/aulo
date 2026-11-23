package aulo.chat.server

import aulo.v1.ConverseRequest
import aulo.v1.ConverseResponse
import aulo.v1.Event
import aulo.v1.Notice
import aulo.v1.NoticeLevel
import aulo.v1.SessionService
import aulo.v1.TextDelta
import aulo.v1.TurnEndReason
import aulo.v1.TurnFinished
import aulo.v1.TurnStarted
import aulo.v1.invoke
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.ProducerScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.rpc.grpc.GrpcStatusCode

const val MAX_TURN_BYTES = 64 * 1024

private const val SOURCE = "aulo-chat-dev-server"

/** Marks a cancellation the user asked for, as opposed to the client going away. */
private class TurnInterrupted : CancellationException("turn interrupted")

/**
 * `Converse` for one chat at a time. A turn passes the rate limiter, waits in the request
 * queue, then streams from the model with retries; its events go only to the stream that sent
 * the turn, since the stand-in does not fan a chat out to several attached clients.
 */
class SessionServiceImpl(
    private val store: ChatStore,
    private val model: ChatModel,
    private val limiter: RateLimiter,
    private val queue: RequestQueue,
    private val retry: RetryPolicy,
) : SessionService {
    override fun converse(message: Flow<ConverseRequest>): Flow<ConverseResponse> = channelFlow {
        var chatId: String? = null
        var turn: Job? = null
        message.collect { request ->
            val submission = request.submission ?: throw invalid("empty submission")
            if (submission is ConverseRequest.Submission.Attach) {
                if (chatId != null) throw status(GrpcStatusCode.FAILED_PRECONDITION, "the stream is already attached")
                val id = submission.value.chatId
                store.chat(id) ?: throw status(GrpcStatusCode.NOT_FOUND, "no chat $id")
                chatId = id
                return@collect
            }
            val attached = chatId
                ?: throw status(GrpcStatusCode.FAILED_PRECONDITION, "the first message must be Attach")
            when (submission) {
                is ConverseRequest.Submission.UserTurn -> {
                    val text = submission.value.text
                    if (text.isBlank()) throw invalid("the turn has no text")
                    if (text.toByteArray().size > MAX_TURN_BYTES) throw invalid("the turn is longer than 64 KiB")
                    if (turn?.isActive == true) {
                        notice(NoticeLevel.WARN, "A turn is already running; stop it before sending another.")
                    } else {
                        turn = launch { runTurn(attached, text) }
                    }
                }

                is ConverseRequest.Submission.Interrupt -> {
                    if (submission.value.all) {
                        // Fail closed: the kill switch needs the `control` scope and the
                        // stand-in has no authentication to grant it with.
                        notice(NoticeLevel.ERROR, "Stopping every chat needs the control scope; only this chat was stopped.")
                    }
                    turn?.cancel(TurnInterrupted())
                }

                is ConverseRequest.Submission.SwitchModel -> {
                    val wanted = submission.value.model.toId()
                    val spec = model.models.firstOrNull { it.id == wanted }
                    if (spec == null || store.setModel(attached, wanted) == null) {
                        notice(NoticeLevel.ERROR, "Unknown model $wanted; the chat keeps its model.")
                    } else {
                        notice(NoticeLevel.INFO, "${spec.displayName} answers from the next turn on.")
                    }
                }

                is ConverseRequest.Submission.DecideApproval, is ConverseRequest.Submission.TakeoverDone ->
                    notice(NoticeLevel.WARN, "Nothing is waiting for that answer: the stub model calls no tools.")

                is ConverseRequest.Submission.Attach -> error("handled above")
            }
        }
        // The client closed its half of the stream; let a running turn finish its answer.
        turn?.join()
    }

    private suspend fun ProducerScope<ConverseResponse>.runTurn(chatId: String, text: String) {
        val turnId = store.newId()
        event { kind = Event.Kind.TurnStarted(TurnStarted { this.turnId = turnId }) }

        limiter.tryAcquire(chatId)?.let { wait ->
            val seconds = (wait.inWholeMilliseconds + 999) / 1000
            return refuse(turnId, "Rate limit reached for this chat; try again in $seconds s.")
        }
        val answer = StringBuilder()
        try {
            queue.run(onQueued = { position -> notice(NoticeLevel.INFO, "Queued: position $position.") }) {
                val chat = store.chat(chatId) ?: return@run refuse(turnId, "The chat was deleted.")
                store.append(chatId, Role.USER, text)
                model.generate(chat.model, store.messages(chatId))
                    .retryTransient(retry) { attempt, wait, cause ->
                        notice(
                            NoticeLevel.WARN,
                            "${cause.message}; retry $attempt of ${retry.maxAttempts - 1} in ${wait.inWholeMilliseconds} ms.",
                        )
                    }
                    .collect { token ->
                        answer.append(token)
                        event {
                            kind = Event.Kind.TextDelta(
                                TextDelta {
                                    this.turnId = turnId
                                    this.text = token
                                },
                            )
                        }
                    }
                store.append(chatId, Role.ASSISTANT, answer.toString())
                finish(turnId, TurnEndReason.COMPLETED)
            }
        } catch (e: QueueFullException) {
            refuse(turnId, "The server is busy (${e.capacity} turns waiting); try again shortly.")
        } catch (e: TurnInterrupted) {
            // What was already shown stays part of the chat, as it does on the client.
            withContext(NonCancellable) {
                if (answer.isNotEmpty()) store.append(chatId, Role.ASSISTANT, answer.toString())
                finish(turnId, TurnEndReason.INTERRUPTED)
            }
            throw e
        } catch (e: TransientModelException) {
            finish(turnId, TurnEndReason.FAILED, "The model is unavailable after ${retry.maxAttempts} attempts: ${e.message}")
        } catch (e: ModelException) {
            finish(turnId, TurnEndReason.FAILED, e.message)
        }
    }

    private suspend fun ProducerScope<ConverseResponse>.refuse(turnId: String, reason: String) {
        notice(NoticeLevel.WARN, reason)
        finish(turnId, TurnEndReason.FAILED, reason)
    }

    private suspend fun ProducerScope<ConverseResponse>.finish(
        turnId: String,
        reason: TurnEndReason,
        error: String? = null,
    ) = event {
        kind = Event.Kind.TurnFinished(
            TurnFinished {
                this.turnId = turnId
                this.reason = reason
                if (error != null) this.error = error
            },
        )
    }

    private suspend fun ProducerScope<ConverseResponse>.notice(level: NoticeLevel, text: String) = event {
        kind = Event.Kind.Notice(
            Notice {
                this.level = level
                message = text
                source = SOURCE
            },
        )
    }

    private suspend fun ProducerScope<ConverseResponse>.event(build: Event.Builder.() -> Unit) {
        send(ConverseResponse { event = Event(build) })
    }
}
