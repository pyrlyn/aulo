package aulo.types

import kotlinx.serialization.Serializable
import kotlin.jvm.JvmInline

// One type per id, so a ChatId can never be passed where a TurnId is expected (spec §5).

@Serializable
@JvmInline
value class BotId(
    val value: String,
) {
    init {
        requireUlid(value)
    }

    override fun toString(): String = value

    companion object {
        fun new(): BotId = BotId(Ulid.next())
    }
}

@Serializable
@JvmInline
value class ChatId(
    val value: String,
) {
    init {
        requireUlid(value)
    }

    override fun toString(): String = value

    companion object {
        fun new(): ChatId = ChatId(Ulid.next())
    }
}

@Serializable
@JvmInline
value class TurnId(
    val value: String,
) {
    init {
        requireUlid(value)
    }

    override fun toString(): String = value

    companion object {
        fun new(): TurnId = TurnId(Ulid.next())
    }
}

@Serializable
@JvmInline
value class CallId(
    val value: String,
) {
    init {
        requireUlid(value)
    }

    override fun toString(): String = value

    companion object {
        fun new(): CallId = CallId(Ulid.next())
    }
}

@Serializable
@JvmInline
value class ClientId(
    val value: String,
) {
    init {
        requireUlid(value)
    }

    override fun toString(): String = value

    companion object {
        fun new(): ClientId = ClientId(Ulid.next())
    }
}
