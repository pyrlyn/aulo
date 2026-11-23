package aulo.secrets

/** A secret could not be saved, read or removed. Messages never contain the secret. */
class SecretException(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause)

/**
 * Named secrets of one aulo home. Values never pass through logs or messages; callers hand
 * them to the transport or the user, never to a model (spec §8).
 */
interface SecretStore {
    /** Where the secrets live, for messages: "the macOS Keychain", a file path. */
    val description: String

    /** Saves [value] under [name], replacing an earlier one. */
    fun save(
        name: String,
        value: String,
    )

    /** The value under [name], or null when there is none. */
    fun load(name: String): String?

    /** Removes [name]; true when there was something to remove. */
    fun delete(name: String): Boolean

    companion object {
        /** Longest secret in UTF-8 bytes: a token or an API key, never a document. */
        const val MAX_VALUE_BYTES = 4096

        private val NAME = Regex("[a-z0-9-]{1,64}")

        /** Names are short and plain, so every backend can use them as a file name or an account. */
        fun checkName(name: String) {
            if (!NAME.matches(name)) throw SecretException("a secret name is 1 to 64 of a-z, 0-9 and -")
        }

        fun checkValue(value: String) {
            if (value.isEmpty() || value.encodeToByteArray().size > MAX_VALUE_BYTES || '\u0000' in value) {
                throw SecretException("a secret is 1 to $MAX_VALUE_BYTES bytes of text without NUL")
            }
        }
    }
}

/**
 * The OS keychain for the aulo home [namespace] (its absolute path keeps two homes apart), or
 * where that has none, 0600 files in [fallbackDir]. [warn] hears when the fallback is used.
 */
expect fun systemSecretStore(
    namespace: String,
    fallbackDir: String,
    warn: (String) -> Unit,
): SecretStore

/** The keychain account of [name] in [namespace]. */
internal fun account(
    namespace: String,
    name: String,
): String = "$name@$namespace"

/**
 * [primary] while it works; [fallback] when it fails. Extensions of the OS (a missing Secret
 * Service, a locked keychain) are warned about, never fatal.
 */
internal class FallbackSecretStore(
    private val primary: SecretStore,
    private val fallback: SecretStore,
    private val warn: (String) -> Unit,
) : SecretStore {
    override val description get() = "${primary.description}, or ${fallback.description}"

    override fun save(
        name: String,
        value: String,
    ) {
        try {
            primary.save(name, value)
            // A copy left by an earlier fallback would shadow nothing but still sit on disk.
            fallback.delete(name)
        } catch (e: SecretException) {
            warn("cannot use ${primary.description} (${e.message}); saving $name to ${fallback.description}")
            fallback.save(name, value)
        }
    }

    override fun load(name: String): String? {
        val found =
            try {
                primary.load(name)
            } catch (e: SecretException) {
                warn("cannot use ${primary.description} (${e.message}); reading $name from ${fallback.description}")
                null
            }
        return found ?: fallback.load(name)
    }

    override fun delete(name: String): Boolean {
        val inPrimary =
            try {
                primary.delete(name)
            } catch (e: SecretException) {
                warn("cannot use ${primary.description} (${e.message})")
                false
            }
        return fallback.delete(name) || inPrimary
    }
}
