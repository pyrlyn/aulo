package aulo.app

/** The `--version` text both executables print: name, version, commit, licence. */
fun versionText(program: String): String = "$program ${BuildInfo.VERSION} (${BuildInfo.GIT_SHA})\nLicense: ${BuildInfo.LICENSE}"
