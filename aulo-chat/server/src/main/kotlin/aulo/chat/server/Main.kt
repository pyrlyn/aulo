package aulo.chat.server

import aulo.v1.ChatService
import aulo.v1.ConfigService
import aulo.v1.SessionService
import kotlinx.coroutines.runBlocking
import kotlinx.rpc.grpc.server.GrpcServer
import kotlinx.rpc.registerService
import kotlin.system.exitProcess

const val DEFAULT_PORT = 50051

/** Everything the development server is made of; tests build it with their own parts. */
class DevServer(
    val model: ChatModel = StubModel(),
    val store: ChatStore = ChatStore(),
    val limiter: RateLimiter = RateLimiter(),
    val queue: RequestQueue = RequestQueue(),
    val retry: RetryPolicy = RetryPolicy(),
) {
    fun grpc(port: Int): GrpcServer = GrpcServer(port) {
        services {
            registerService<ChatService> { ChatServiceImpl(store, model, limiter) }
            registerService<SessionService> { SessionServiceImpl(store, model, limiter, queue, retry) }
            registerService<ConfigService> { ConfigServiceImpl(model) }
        }
    }
}

fun main() {
    val raw = System.getenv("AULO_CHAT_PORT")
    val port = if (raw.isNullOrBlank()) DEFAULT_PORT else raw.toIntOrNull()?.takeIf { it in 1..65535 }
    if (port == null) {
        System.err.println("AULO_CHAT_PORT must be a port number, got \"$raw\"")
        exitProcess(2)
    }
    val server = DevServer().grpc(port).start()
    Runtime.getRuntime().addShutdownHook(Thread { server.shutdownNow() })
    println("aulo-chat development server on port ${server.port}: plaintext gRPC, no authentication, stub model.")
    println("It listens on every interface; run it only on a machine and network you trust.")
    runBlocking { server.awaitTermination() }
}
