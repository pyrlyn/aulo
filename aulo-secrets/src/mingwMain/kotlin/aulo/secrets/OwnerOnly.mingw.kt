@file:OptIn(ExperimentalForeignApi::class)

package aulo.secrets

import kotlinx.cinterop.ExperimentalForeignApi
import platform.windows.CreateDirectoryW
import platform.windows.ERROR_ALREADY_EXISTS
import platform.windows.GetLastError

// Windows has no POSIX modes: what is created here inherits the ACL of its parent, which under
// the user profile grants access to the user, SYSTEM and administrators only.

actual fun ensureOwnerOnlyDir(dir: String) {
    if (CreateDirectoryW(dir, null) != 0) return
    val error = GetLastError()
    if (error != ERROR_ALREADY_EXISTS.toUInt()) throw OwnerOnlyException("cannot create $dir: Windows error $error")
}

actual fun restrictToOwner(file: String) = Unit
