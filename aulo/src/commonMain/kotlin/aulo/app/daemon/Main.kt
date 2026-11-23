package aulo.app.daemon

import aulo.app.versionText

// Until the daemon tasks (S3) land, every invocation only reports the build.
fun main() {
    println(versionText("aulod"))
}
