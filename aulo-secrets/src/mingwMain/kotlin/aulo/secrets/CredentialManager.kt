@file:OptIn(ExperimentalForeignApi::class)

package aulo.secrets

import aulo.secrets.wincred.CREDENTIALW
import aulo.secrets.wincred.CRED_PERSIST_LOCAL_MACHINE
import aulo.secrets.wincred.CRED_TYPE_GENERIC
import aulo.secrets.wincred.CredDeleteW
import aulo.secrets.wincred.CredFree
import aulo.secrets.wincred.CredReadW
import aulo.secrets.wincred.CredWriteW
import aulo.secrets.wincred.PCREDENTIALWVar
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.allocArrayOf
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.pointed
import kotlinx.cinterop.ptr
import kotlinx.cinterop.readBytes
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.value
import kotlinx.cinterop.wcstr
import platform.windows.ERROR_NOT_FOUND
import platform.windows.GetLastError

actual fun systemSecretStore(
    namespace: String,
    fallbackDir: String,
    warn: (String) -> Unit,
): SecretStore = FallbackSecretStore(CredentialManagerSecretStore(namespace), FileSecretStore(fallbackDir), warn)

/** Generic credentials of the user in Windows Credential Manager, target `aulo/<name>@<namespace>`. */
class CredentialManagerSecretStore(
    private val namespace: String,
) : SecretStore {
    override val description get() = "Windows Credential Manager"

    override fun save(
        name: String,
        value: String,
    ) {
        SecretStore.checkName(name)
        SecretStore.checkValue(value)
        val bytes = value.encodeToByteArray()
        memScoped {
            val credential = alloc<CREDENTIALW>()
            credential.Type = CRED_TYPE_GENERIC.toUInt()
            credential.TargetName = target(name).wcstr.ptr
            credential.UserName = "aulo".wcstr.ptr
            credential.CredentialBlobSize = bytes.size.toUInt()
            credential.CredentialBlob = allocArrayOf(bytes).reinterpret()
            credential.Persist = CRED_PERSIST_LOCAL_MACHINE.toUInt()
            if (CredWriteW(credential.ptr, 0u) == 0) throw SecretException("cannot save $name (Windows error ${GetLastError()})")
        }
    }

    override fun load(name: String): String? {
        SecretStore.checkName(name)
        return memScoped {
            val out = alloc<PCREDENTIALWVar>()
            if (CredReadW(target(name), CRED_TYPE_GENERIC.toUInt(), 0u, out.ptr) == 0) {
                val error = GetLastError()
                if (error == ERROR_NOT_FOUND.toUInt()) return@memScoped null
                throw SecretException("cannot read $name (Windows error $error)")
            }
            val credential = out.value ?: return@memScoped null
            try {
                val c = credential.pointed
                val size = c.CredentialBlobSize.toInt()
                if (size > SecretStore.MAX_VALUE_BYTES) throw SecretException("the credential for $name is too large")
                if (size == 0) null else c.CredentialBlob?.readBytes(size)?.decodeToString()
            } finally {
                CredFree(credential)
            }
        }
    }

    override fun delete(name: String): Boolean {
        SecretStore.checkName(name)
        if (CredDeleteW(target(name), CRED_TYPE_GENERIC.toUInt(), 0u) != 0) return true
        val error = GetLastError()
        if (error == ERROR_NOT_FOUND.toUInt()) return false
        throw SecretException("cannot remove $name (Windows error $error)")
    }

    private fun target(name: String) = "aulo/" + account(namespace, name)
}
