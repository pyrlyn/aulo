@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

package aulo.speech.sherpa

import aulo.speech.helper.FrameType
import aulo.speech.helper.WireException
import aulo.speech.helper.serveStt
import aulo.speech.helper.writeText
import okio.buffer
import kotlin.system.exitProcess

private const val USAGE = "usage: aulo-speech-sherpa stt <parakeet|moonshine> <model dir> <threads>"
private const val EXIT_USAGE = 2

/**
 * The sherpa-onnx helper. aulod starts it per engine and speaks the pipe protocol of
 * :aulo-speech-helper on its stdin and stdout; stderr is for people only. It never writes
 * transcripts to stderr, since aulod may log that.
 */
fun main(args: Array<String>) {
    binaryStdio()
    val model = args.getOrNull(1)?.let(SttModel::of)
    val threads = args.getOrNull(3)?.toIntOrNull()?.takeIf { it in 1..MAX_THREADS }
    if (args.size != 4 || args[0] != "stt" || model == null || threads == null) {
        printErr(USAGE)
        exitProcess(EXIT_USAGE)
    }
    val output = StdoutSink().buffer()
    val recognizer =
        try {
            OfflineRecognizer(model, args[2], threads)
        } catch (e: SherpaException) {
            // aulod reports this as the engine being unavailable and falls back.
            output.writeText(FrameType.ERROR, 0, e.message.orEmpty())
            output.flush()
            exitProcess(1)
        }
    try {
        serveStt(StdinSource().buffer(), output, recognizer, model.serve)
    } catch (e: WireException) {
        printErr("aulo-speech-sherpa: ${e.message}")
        exitProcess(1)
    } finally {
        recognizer.close()
    }
}

private fun printErr(text: String) {
    platform.posix.fputs("$text\n", platform.posix.stderr)
}
