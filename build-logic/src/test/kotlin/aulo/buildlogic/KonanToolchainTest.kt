package aulo.buildlogic

import java.util.Properties
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class KonanToolchainTest {
    // The shape of the lines of Kotlin/Native 2.4.21's konan.properties that the build reads.
    private val properties =
        Properties().apply {
            """
            toolchainDependency.mingw_x64 = msys2-mingw-w64-x86_64-2
            toolchainDependency.linux_x64 = x86_64-unknown-linux-gnu-gcc-8.3.0-glibc-2.19-kernel-4.9-2
            llvm.macos_arm64.user = llvm-21-aarch64-macos-essentials-97
            llvmHome.macos_arm64 = ${'$'}llvm.macos_arm64.user
            gccToolchain.linux_x64 = ${'$'}toolchainDependency.linux_x64
            targetSysRoot.linux_x64 = ${'$'}gccToolchain.linux_x64/x86_64-unknown-linux-gnu/sysroot
            targetSysRoot.mingw_x64 = ${'$'}toolchainDependency.mingw_x64
            loop.a = ${'$'}loop.b
            loop.b = ${'$'}loop.a/x
            dangling = ${'$'}nowhere
            """.trimIndent().reader().use(::load)
        }

    @Test
    fun referencesAreFollowedAndKeepWhatComesAfterTheName() {
        assertEquals("llvm-21-aarch64-macos-essentials-97", KonanToolchain.resolve(properties, "llvmHome.macos_arm64"))
        assertEquals(
            "x86_64-unknown-linux-gnu-gcc-8.3.0-glibc-2.19-kernel-4.9-2/x86_64-unknown-linux-gnu/sysroot",
            KonanToolchain.resolve(properties, "targetSysRoot.linux_x64"),
        )
        assertNull(KonanToolchain.resolve(properties, "gccToolchain.mingw_x64"))
    }

    @Test
    fun aLoopOrADanglingReferenceIsAnError() {
        assertFailsWith<IllegalStateException> { KonanToolchain.resolve(properties, "loop.a") }
        assertFailsWith<IllegalStateException> { KonanToolchain.resolve(properties, "dangling") }
    }

    @Test
    fun aLinuxTargetGetsItsGccToolchainAndWindowsDoesNot() {
        assertEquals(
            mapOf(
                "KONAN_LLVM" to "/k/dependencies/llvm-21-aarch64-macos-essentials-97",
                "KONAN_SYSROOT" to
                    "/k/dependencies/x86_64-unknown-linux-gnu-gcc-8.3.0-glibc-2.19-kernel-4.9-2/x86_64-unknown-linux-gnu/sysroot",
                "KONAN_GCC_TOOLCHAIN" to "/k/dependencies/x86_64-unknown-linux-gnu-gcc-8.3.0-glibc-2.19-kernel-4.9-2",
            ),
            KonanToolchain.environment(properties, "/k/dependencies", "macos_arm64", "linux_x64"),
        )
        assertEquals(
            mapOf(
                "KONAN_LLVM" to "/k/dependencies/llvm-21-aarch64-macos-essentials-97",
                "KONAN_SYSROOT" to "/k/dependencies/msys2-mingw-w64-x86_64-2",
            ),
            KonanToolchain.environment(properties, "/k/dependencies", "macos_arm64", "mingw_x64"),
        )
    }

    @Test
    fun aHostWithoutLlvmInThePropertiesIsAnError() {
        assertFailsWith<IllegalStateException> { KonanToolchain.environment(properties, "/k/dependencies", "linux_x64", "linux_x64") }
    }

    @Test
    fun hostsAreNamedAsKotlinNativeNamesThem() {
        assertEquals("macos_arm64", KonanToolchain.hostName("Mac OS X"))
        assertEquals("mingw_x64", KonanToolchain.hostName("Windows Server 2025"))
        assertEquals("linux_x64", KonanToolchain.hostName("Linux"))
    }
}
