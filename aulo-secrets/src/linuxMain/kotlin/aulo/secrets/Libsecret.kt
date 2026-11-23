@file:OptIn(ExperimentalForeignApi::class)

package aulo.secrets

import aulo.secrets.libsecret.aulo_secret_clear
import aulo.secrets.libsecret.aulo_secret_free
import aulo.secrets.libsecret.aulo_secret_lookup
import aulo.secrets.libsecret.aulo_secret_open
import aulo.secrets.libsecret.aulo_secret_store
import kotlinx.cinterop.ByteVar
import kotlinx.cinterop.CPointerVar
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.toKString
import kotlinx.cinterop.value
import platform.posix.free
import platform.posix.strlen

actual fun systemSecretStore(
    namespace: String,
    fallbackDir: String,
    warn: (String) -> Unit,
): SecretStore = FallbackSecretStore(LibsecretSecretStore(namespace), FileSecretStore(fallbackDir), warn)

/**
 * The Secret Service (GNOME Keyring, KWallet) through the system libsecret, loaded at run time.
 * Items use schema `app.aulo.Secret` with attribute `account` = `<name>@<namespace>`.
 */
class LibsecretSecretStore(
    private val namespace: String,
) : SecretStore {
    override val description get() = "the Secret Service"

    override fun save(
        name: String,
        value: String,
    ) {
        SecretStore.checkName(name)
        SecretStore.checkValue(value)
        loaded()
        memScoped {
            val error = alloc<CPointerVar<ByteVar>>()
            val ok = aulo_secret_store(account(namespace, name), "aulo $name", value, error.ptr)
            failure(error)?.let { throw SecretException("cannot save $name: $it") }
            if (ok == 0) throw SecretException("cannot save $name")
        }
    }

    override fun load(name: String): String? {
        SecretStore.checkName(name)
        loaded()
        return memScoped {
            val error = alloc<CPointerVar<ByteVar>>()
            val found = aulo_secret_lookup(account(namespace, name), error.ptr)
            failure(error)?.let {
                aulo_secret_free(found)
                throw SecretException("cannot read $name: $it")
            }
            if (found == null) return@memScoped null
            try {
                if (strlen(found).toLong() > SecretStore.MAX_VALUE_BYTES) throw SecretException("the item for $name is too large")
                found.toKString().ifEmpty { null }
            } finally {
                aulo_secret_free(found)
            }
        }
    }

    override fun delete(name: String): Boolean {
        SecretStore.checkName(name)
        loaded()
        return memScoped {
            val error = alloc<CPointerVar<ByteVar>>()
            val removed = aulo_secret_clear(account(namespace, name), error.ptr)
            failure(error)?.let { throw SecretException("cannot remove $name: $it") }
            removed != 0
        }
    }

    private fun loaded() {
        unavailable?.let { throw SecretException(it) }
    }

    /** The message libsecret reported, freed here; null when it reported none. */
    private fun failure(error: CPointerVar<ByteVar>): String? {
        val message = error.value ?: return null
        return try {
            message.toKString()
        } finally {
            free(message)
        }
    }

    private companion object {
        /** Why libsecret cannot be used, or null once it is loaded; resolved once per process. */
        val unavailable: String? by lazy { aulo_secret_open()?.toKString() }
    }
}
