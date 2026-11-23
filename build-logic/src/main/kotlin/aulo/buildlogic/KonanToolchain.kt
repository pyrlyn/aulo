package aulo.buildlogic

import java.io.File
import java.util.Properties

/**
 * The compiler and sysroot Kotlin/Native builds one target with, read from its `konan.properties`.
 * Conan compiles C libraries with the same ones ([ConanInstallTask]), so a library never needs a
 * newer libc than the binary it is linked into.
 */
object KonanToolchain {
    /** Kotlin/Native's name of the machine the build runs on, the way `konan.properties` keys end. */
    fun hostName(osName: String): String =
        when {
            osName.startsWith("Mac") -> "macos_arm64"
            osName.startsWith("Windows") -> "mingw_x64"
            else -> "linux_x64"
        }

    /** A value with its `$other.key/rest` references followed, as Kotlin/Native follows them. */
    fun resolve(
        properties: Properties,
        key: String,
        seen: Set<String> = emptySet(),
    ): String? {
        check(key !in seen) { "konan.properties: $key refers to itself" }
        val value = properties.getProperty(key)?.trim() ?: return null
        if (!value.startsWith("$")) return value
        val name = value.substringBefore('/').drop(1)
        val referred = resolve(properties, name, seen + key) ?: error("konan.properties: $key refers to $name, which is not set")
        return referred + value.substring(name.length + 1)
    }

    /**
     * What `conan/<target>.profile` reads from the environment: the host's LLVM, the target's
     * sysroot and, for a Linux target, its GCC toolchain. [dependencies] is the directory
     * Kotlin/Native unpacks them into; [host] and [target] are its names (`linux_x64`).
     */
    fun environment(
        properties: Properties,
        dependencies: String,
        host: String,
        target: String,
    ): Map<String, String> {
        fun path(key: String): String? = resolve(properties, key)?.let { if (File(it).isAbsolute) it else "$dependencies/$it" }
        return buildMap {
            put("KONAN_LLVM", path("llvmHome.$host") ?: error("konan.properties has no llvmHome.$host"))
            put("KONAN_SYSROOT", path("targetSysRoot.$target") ?: error("konan.properties has no targetSysRoot.$target"))
            path("gccToolchain.$target")?.let { put("KONAN_GCC_TOOLCHAIN", it) }
        }
    }
}
