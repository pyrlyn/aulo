package aulo.app.cli

import aulo.app.versionText

// Until the CLI tasks (S3) land, every invocation only reports the build.
fun main() {
    println(versionText("aulo"))
}
