@file:OptIn(ExperimentalForeignApi::class)

package aulo.speech.sherpa

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.convert
import kotlinx.cinterop.toKString
import kotlinx.cinterop.usePinned
import okio.IOException
import platform.posix.EINTR
import platform.posix.STDIN_FILENO
import platform.posix.STDOUT_FILENO
import platform.posix.errno
import platform.posix.read
import platform.posix.strerror
import platform.posix.write

internal actual fun readStdin(
    buf: ByteArray,
    max: Int,
): Int {
    while (true) {
        val n: Long = buf.usePinned { read(STDIN_FILENO, it.addressOf(0), max.convert()) }.convert()
        if (n >= 0) return n.toInt()
        if (errno != EINTR) throw IOException("stdin: ${strerror(errno)?.toKString()}")
    }
}

internal actual fun writeStdout(
    buf: ByteArray,
    count: Int,
) {
    var done = 0
    while (done < count) {
        val n: Long = buf.usePinned { write(STDOUT_FILENO, it.addressOf(done), (count - done).convert()) }.convert()
        if (n < 0) {
            if (errno == EINTR) continue
            throw IOException("stdout: ${strerror(errno)?.toKString()}")
        }
        done += n.toInt()
    }
}

internal actual fun binaryStdio() {}
