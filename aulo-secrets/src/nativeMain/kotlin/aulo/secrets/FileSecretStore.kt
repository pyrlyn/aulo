package aulo.secrets

import fs.EntryKind
import fs.FsException
import fs.WriteOptions
import fs.entryKind
import fs.linkSize
import fs.readText
import fs.removeFile
import kotlinx.io.files.Path

/**
 * Secrets as 0600 files in the owner-only directory [dir], one per name: for systems without
 * a keychain, such as a server without a Secret Service.
 */
class FileSecretStore(
    private val dir: String,
) : SecretStore {
    override val description get() = "files in $dir"

    override fun save(
        name: String,
        value: String,
    ) {
        SecretStore.checkName(name)
        SecretStore.checkValue(value)
        prepare()
        val path = path(name)
        try {
            // Not followed: a link planted in place of the file would send the secret elsewhere.
            WriteOptions(preservePermissions = false, followSymlinks = false, mode = OWNER_ONLY_FILE).write(path, value.encodeToByteArray())
            restrictToOwner(path.toString())
        } catch (e: FsException) {
            throw SecretException("cannot save $name to $dir: ${e.message}", e)
        } catch (e: OwnerOnlyException) {
            throw SecretException("cannot save $name to $dir: ${e.message}", e)
        }
    }

    override fun load(name: String): String? {
        SecretStore.checkName(name)
        val path = path(name)
        val kind = entryKind(path) ?: return null
        if (kind != EntryKind.File) throw SecretException("$path is not a plain file")
        prepare()
        val size = linkSize(path) ?: return null
        if (size > SecretStore.MAX_VALUE_BYTES) throw SecretException("$path is larger than ${SecretStore.MAX_VALUE_BYTES} bytes")
        return try {
            readText(path).ifEmpty { null }
        } catch (e: FsException) {
            throw SecretException("cannot read $name from $dir: ${e.message}", e)
        }
    }

    override fun delete(name: String): Boolean {
        SecretStore.checkName(name)
        val path = path(name)
        if (entryKind(path) == null) return false
        return try {
            removeFile(path)
            true
        } catch (e: FsException) {
            throw SecretException("cannot remove $name from $dir: ${e.message}", e)
        }
    }

    private fun prepare() {
        try {
            ensureOwnerOnlyDir(dir)
        } catch (e: OwnerOnlyException) {
            throw SecretException(e.message ?: "cannot prepare $dir", e)
        }
    }

    private fun path(name: String) = Path(dir, name)

    private companion object {
        const val OWNER_ONLY_FILE = 0b110_000_000
    }
}
