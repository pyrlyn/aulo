import aulo.buildlogic.ModuleRole

// Which speech engine runs (spec §4.2, §5.1): the registry of engine factories, the candidates
// a chat, bot and config pick, and the engine in use with fallback and mid-chat switching. Also the text side
// of speech (T7.15): model markdown in, speakable sentences out.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.LIBRARY
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            api(project(":aulo-speech"))
            implementation(libs.jetbrains.markdown)
        }
        commonTest.dependencies {
            implementation(project(":aulo-testkit"))
        }
        nativeTest.dependencies {
            implementation("pyrlyn.kit:fs")
        }
    }
}

// GoldenTest compares spoken text with the snapshots in tests/snapshots; AULO_UPDATE_FIXTURES=1 rewrites them.
val snapshots = layout.projectDirectory.dir("tests/snapshots")
val updateFixtures = providers.environmentVariable("AULO_UPDATE_FIXTURES").orElse("")
tasks.withType<org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest>().configureEach {
    inputs.dir(snapshots)
    inputs.property("updateFixtures", updateFixtures)
    environment("AULO_VOICE_SNAPSHOTS", snapshots.asFile.path)
    environment("AULO_UPDATE_FIXTURES", updateFixtures.get())
}
