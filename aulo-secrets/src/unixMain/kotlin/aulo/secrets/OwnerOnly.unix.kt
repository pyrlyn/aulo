@file:OptIn(ExperimentalForeignApi::class)

package aulo.secrets

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.convert
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.toKString
import platform.posix.EEXIST
import platform.posix.ENOENT
import platform.posix.S_IFDIR
import platform.posix.S_IFMT
import platform.posix.chmod
import platform.posix.errno
import platform.posix.getuid
import platform.posix.lstat
import platform.posix.mkdir
import platform.posix.stat
import platform.posix.strerror

private const val OWNER_ONLY_DIR = 0b111_000_000
private const val OWNER_ONLY_FILE = 0b110_000_000
private const val GROUP_AND_OTHER = 0b000_111_111

actual fun ensureOwnerOnlyDir(dir: String) {
    // Created with its final mode: a chmod after mkdir would leave a window to cd into it.
    if (mkdir(dir, OWNER_ONLY_DIR.convert()) == 0) return
    val created = errno
    if (created != EEXIST) {
        val hint = if (created == ENOENT) " (its parent does not exist)" else ""
        throw OwnerOnlyException("cannot create $dir: ${reason(created)}$hint")
    }
    memScoped {
        val st = alloc<stat>()
        if (lstat(dir, st.ptr) != 0) throw OwnerOnlyException("cannot inspect $dir: ${reason(errno)}")
        val mode = st.st_mode.toInt()
        if (mode and S_IFMT != S_IFDIR) throw OwnerOnlyException("$dir must be a directory, not a link or a file")
        if (st.st_uid != getuid()) throw OwnerOnlyException("$dir belongs to another user")
        if (mode and GROUP_AND_OTHER != 0) throw OwnerOnlyException("$dir is open to other users; run chmod 700 $dir")
    }
}

actual fun restrictToOwner(file: String) {
    if (chmod(file, OWNER_ONLY_FILE.convert()) != 0) throw OwnerOnlyException("cannot restrict $file: ${reason(errno)}")
}

private fun reason(code: Int): String = strerror(code)?.toKString() ?: "error $code"
