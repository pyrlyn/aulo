@file:OptIn(ExperimentalForeignApi::class)

package aulo.speech.sherpa

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.convert
import kotlinx.cinterop.toKString
import kotlinx.cinterop.usePinned
import okio.IOException
import platform.posix.EINTR
import platform.posix._O_BINARY
import platform.posix._setmode
import platform.posix.errno
import platform.posix.read
import platform.posix.strerror
import platform.posix.write

private const val STDIN = 0
private const val STDOUT = 1

internal actual fun readStdin(
    buf: ByteArray,
    max: Int,
): Int {
    while (true) {
        val n: Int = buf.usePinned { read(STDIN, it.addressOf(0), max.convert()) }
        if (n >= 0) return n
        if (errno != EINTR) throw IOException("stdin: ${strerror(errno)?.toKString()}")
    }
}

internal actual fun writeStdout(
    buf: ByteArray,
    count: Int,
) {
    var done = 0
    while (done < count) {
        val n: Int = buf.usePinned { write(STDOUT, it.addressOf(done), (count - done).convert()) }
        if (n < 0) {
            if (errno == EINTR) continue
            throw IOException("stdout: ${strerror(errno)?.toKString()}")
        }
        done += n
    }
}

internal actual fun binaryStdio() {
    _setmode(STDIN, _O_BINARY)
    _setmode(STDOUT, _O_BINARY)
}
