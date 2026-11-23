@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

package aulo.speech.system

import aulo.speech.helper.StdinSource
import aulo.speech.helper.StdoutSink
import aulo.speech.helper.WireException
import aulo.speech.helper.binaryStdio
import aulo.speech.helper.serveTts
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.launch
import okio.IOException
import okio.buffer
import platform.CoreFoundation.CFRunLoopRun
import kotlin.system.exitProcess

private const val USAGE = "usage: aulo-speech-system tts"
private const val EXIT_USAGE = 2
private const val EXIT_HUNG = 3

/**
 * The system-voices helper. aulod starts it per engine and speaks the pipe protocol of
 * :aulo-speech-helper on its stdin and stdout; stderr is for people only, and never carries the
 * text it speaks, since aulod may log that.
 */
fun main(args: Array<String>) {
    binaryStdio()
    if (args.toList() != listOf("tts")) {
        printErr(USAGE)
        exitProcess(EXIT_USAGE)
    }
    val synth = AppleSynth()
    // AVSpeechSynthesizer hands its audio over on the main run loop, so the main thread runs that
    // loop and the protocol runs on an I/O thread, which ends the process when it is done.
    CoroutineScope(Dispatchers.IO).launch {
        val code =
            try {
                serveTts(StdinSource().buffer(), StdoutSink().buffer(), synth)
                0
            } catch (e: SynthHungException) {
                printErr("aulo-speech-system: ${e.message}")
                EXIT_HUNG
            } catch (e: WireException) {
                printErr("aulo-speech-system: ${e.message}")
                1
            } catch (e: IOException) {
                // aulod closed the pipe; it no longer listens.
                1
            }
        exitProcess(code)
    }
    CFRunLoopRun()
}

private fun printErr(text: String) {
    platform.posix.fputs("$text\n", platform.posix.stderr)
}
