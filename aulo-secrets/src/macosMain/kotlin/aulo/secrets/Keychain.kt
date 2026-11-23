@file:OptIn(ExperimentalForeignApi::class)

package aulo.secrets

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.convert
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.readBytes
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.toCValues
import kotlinx.cinterop.value
import platform.CoreFoundation.CFDataCreate
import platform.CoreFoundation.CFDataGetBytePtr
import platform.CoreFoundation.CFDataGetLength
import platform.CoreFoundation.CFDictionaryAddValue
import platform.CoreFoundation.CFDictionaryCreateMutable
import platform.CoreFoundation.CFMutableDictionaryRef
import platform.CoreFoundation.CFRelease
import platform.CoreFoundation.CFStringCreateWithCString
import platform.CoreFoundation.CFStringRef
import platform.CoreFoundation.CFTypeRef
import platform.CoreFoundation.CFTypeRefVar
import platform.CoreFoundation.kCFBooleanTrue
import platform.CoreFoundation.kCFStringEncodingUTF8
import platform.CoreFoundation.kCFTypeDictionaryKeyCallBacks
import platform.CoreFoundation.kCFTypeDictionaryValueCallBacks
import platform.Security.SecItemAdd
import platform.Security.SecItemCopyMatching
import platform.Security.SecItemDelete
import platform.Security.SecItemUpdate
import platform.Security.errSecDuplicateItem
import platform.Security.errSecItemNotFound
import platform.Security.errSecSuccess
import platform.Security.kSecAttrAccount
import platform.Security.kSecAttrLabel
import platform.Security.kSecAttrService
import platform.Security.kSecClass
import platform.Security.kSecClassGenericPassword
import platform.Security.kSecMatchLimit
import platform.Security.kSecMatchLimitOne
import platform.Security.kSecReturnData
import platform.Security.kSecValueData
import platform.darwin.OSStatus

actual fun systemSecretStore(
    namespace: String,
    fallbackDir: String,
    warn: (String) -> Unit,
): SecretStore = FallbackSecretStore(KeychainSecretStore(namespace), FileSecretStore(fallbackDir), warn)

/** Generic passwords in the user's login keychain: service `aulo`, account `<name>@<namespace>`. */
class KeychainSecretStore(
    private val namespace: String,
) : SecretStore {
    override val description get() = "the macOS Keychain"

    override fun save(
        name: String,
        value: String,
    ) {
        SecretStore.checkName(name)
        SecretStore.checkValue(value)
        val bytes = value.encodeToByteArray()
        val data = CFDataCreate(null, bytes.toUByteArray().toCValues(), bytes.size.convert()) ?: throw SecretException("out of memory")
        try {
            val added =
                Query(namespace, name).use { q ->
                    q.put(kSecValueData, data)
                    q.text(kSecAttrLabel, "aulo $name")
                    SecItemAdd(q.dict, null)
                }
            if (added != errSecDuplicateItem) return check(added, "save", name)
            val update = Dictionary()
            update.use { u ->
                u.put(kSecValueData, data)
                check(Query(namespace, name).use { q -> SecItemUpdate(q.dict, u.dict) }, "save", name)
            }
        } finally {
            CFRelease(data)
        }
    }

    override fun load(name: String): String? {
        SecretStore.checkName(name)
        return memScoped {
            val result = alloc<CFTypeRefVar>()
            val status =
                Query(namespace, name).use { q ->
                    q.put(kSecReturnData, kCFBooleanTrue)
                    q.put(kSecMatchLimit, kSecMatchLimitOne)
                    SecItemCopyMatching(q.dict, result.ptr)
                }
            if (status == errSecItemNotFound) return@memScoped null
            check(status, "read", name)
            val data = result.value?.reinterpret<cnames.structs.__CFData>() ?: return@memScoped null
            try {
                val length = CFDataGetLength(data).toInt()
                if (length > SecretStore.MAX_VALUE_BYTES) throw SecretException("the keychain item for $name is too large")
                if (length == 0) null else CFDataGetBytePtr(data)?.readBytes(length)?.decodeToString()
            } finally {
                CFRelease(data)
            }
        }
    }

    override fun delete(name: String): Boolean {
        SecretStore.checkName(name)
        val status = Query(namespace, name).use { q -> SecItemDelete(q.dict) }
        if (status == errSecItemNotFound) return false
        check(status, "remove", name)
        return true
    }

    private fun check(
        status: OSStatus,
        what: String,
        name: String,
    ) {
        if (status != errSecSuccess) throw SecretException("cannot $what $name in the keychain (OSStatus $status)")
    }

    /** A CF dictionary and the strings created for it, released together. */
    private open class Dictionary : AutoCloseable {
        val dict: CFMutableDictionaryRef =
            CFDictionaryCreateMutable(null, 0, kCFTypeDictionaryKeyCallBacks.ptr, kCFTypeDictionaryValueCallBacks.ptr)
                ?: throw SecretException("out of memory")
        private val owned = ArrayList<CFTypeRef>()

        fun put(
            key: CFStringRef?,
            value: CFTypeRef?,
        ) = CFDictionaryAddValue(dict, key, value)

        fun text(
            key: CFStringRef?,
            value: String,
        ) {
            val string = CFStringCreateWithCString(null, value, kCFStringEncodingUTF8) ?: throw SecretException("out of memory")
            owned += string
            put(key, string)
        }

        override fun close() {
            CFRelease(dict)
            owned.forEach(::CFRelease)
        }
    }

    /** The attributes that name one item: class, service and account. */
    private class Query(
        namespace: String,
        name: String,
    ) : Dictionary() {
        init {
            put(kSecClass, kSecClassGenericPassword)
            text(kSecAttrService, SERVICE)
            text(kSecAttrAccount, account(namespace, name))
        }
    }

    private companion object {
        const val SERVICE = "aulo"
    }
}
