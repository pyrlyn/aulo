package aulo.chat.domain

/** The limit `aulo.v1.UserTurn.text` sets; the server rejects longer text rather than cutting it. */
const val MAX_TURN_BYTES = 64 * 1024

class MessageTooLongException(val bytes: Int) :
    Exception("The message is ${bytes / 1024} KiB; the limit is ${MAX_TURN_BYTES / 1024} KiB.")

/** What [SendMessage] did with the attachments it could not transmit. */
data class SendResult(val notSent: List<Attachment>)

/**
 * Sends one user turn. Text attachments are written into the turn under a header line, so the
 * model reads them; images and voice notes are shown on the message but stay on the device,
 * because the contract has no field to carry them yet.
 */
class SendMessage {
    suspend operator fun invoke(session: ChatSession, input: String, attachments: List<Attachment>): SendResult {
        val text = compose(input, attachments)
        require(text.isNotBlank()) { "nothing to send" }
        val bytes = text.encodeToByteArray().size
        if (bytes > MAX_TURN_BYTES) throw MessageTooLongException(bytes)
        session.send(text, attachments.map { it.copy(content = null) })
        return SendResult(attachments.filter { it.kind != AttachmentKind.Text })
    }

    internal fun compose(input: String, attachments: List<Attachment>): String = buildString {
        append(input.trim())
        for (file in attachments) {
            val content = file.content?.takeIf { file.kind == AttachmentKind.Text } ?: continue
            if (isNotEmpty()) append("\n\n")
            append("--- ").append(file.name).append(" ---\n").append(content.trim())
        }
    }
}

class StopGeneration {
    suspend operator fun invoke(session: ChatSession) = session.stop()
}

class SwitchModel {
    suspend operator fun invoke(session: ChatSession, model: ModelId) = session.switchModel(model)
}

class OpenChat(private val repository: ChatRepository) {
    suspend operator fun invoke(): ChatSummary? = repository.connect()
}

class LoadModels(private val repository: ChatRepository) {
    /** An empty list offline: the picker then shows only the chat's current model. */
    suspend operator fun invoke(): List<ModelInfo> = try {
        repository.models()
    } catch (_: OfflineException) {
        emptyList()
    }
}
