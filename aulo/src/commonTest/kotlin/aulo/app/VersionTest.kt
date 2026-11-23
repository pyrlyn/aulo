package aulo.app

import kotlin.test.Test
import kotlin.test.assertEquals

class VersionTest {
    @Test
    fun namesProgramVersionCommitAndLicence() {
        val lines = versionText("aulod").lines()
        assertEquals("aulod ${BuildInfo.VERSION} (${BuildInfo.GIT_SHA})", lines[0])
        assertEquals("License: GPL-3.0-or-later OR LicenseRef-aulo-Royalty-Free", lines[1])
    }
}
