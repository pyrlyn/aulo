package aulo.secrets

import fs.TempDir
import fs.WriteOptions
import fs.createDir
import fs.mode
import fs.setMode
import fs.symlink
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.io.files.Path
import platform.posix.getenv
import kotlin.experimental.ExperimentalNativeApi
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalNativeApi::class)
private val posix = Platform.osFamily != OsFamily.WINDOWS

class FileSecretStoreTest {
    @Test
    fun a_secret_round_trips_and_is_removed() =
        home { root ->
            val store = FileSecretStore("$root/secrets")
            assertNull(store.load("token"))
            store.save("token", "first")
            store.save("token", "second")
            assertEquals("second", store.load("token"))
            assertTrue(store.delete("token"))
            assertFalse(store.delete("token"))
            assertNull(store.load("token"))
        }

    @Test
    fun the_directory_is_0700_and_the_file_0600() =
        home { root ->
            if (!posix) return@home
            FileSecretStore("$root/secrets").save("token", "t")
            assertEquals(0b111_000_000, mode(Path("$root/secrets"))!! and 0b111_111_111)
            assertEquals(0b110_000_000, mode(Path("$root/secrets/token"))!! and 0b111_111_111)
        }

    @Test
    fun names_and_values_are_checked() =
        home { root ->
            val store = FileSecretStore("$root/secrets")
            for (bad in listOf("", "../x", "a/b", "A", "x".repeat(65), "a b")) {
                assertFailsWith<SecretException>(bad) { store.save(bad, "v") }
            }
            assertFailsWith<SecretException> { store.save("token", "") }
            assertFailsWith<SecretException> { store.save("token", "a\u0000b") }
            store.save("token", "x".repeat(SecretStore.MAX_VALUE_BYTES))
            assertFailsWith<SecretException> { store.save("token", "x".repeat(SecretStore.MAX_VALUE_BYTES + 1)) }
        }

    @Test
    fun an_oversized_file_is_refused_on_read() =
        home { root ->
            val store = FileSecretStore("$root/secrets")
            store.save("token", "t")
            WriteOptions().write(Path("$root/secrets/token"), ByteArray(SecretStore.MAX_VALUE_BYTES + 1) { 'x'.code.toByte() })
            assertFailsWith<SecretException> { store.load("token") }
        }

    @Test
    fun a_link_in_place_of_the_file_is_refused() =
        home { root ->
            if (!posix) return@home
            val store = FileSecretStore("$root/secrets")
            store.save("other", "t")
            symlink(Path("$root/secrets/other"), Path("$root/secrets/token"))
            assertFailsWith<SecretException> { store.load("token") }
        }

    @Test
    fun a_loose_directory_is_refused() =
        home { root ->
            if (!posix) return@home
            createDir(Path("$root/secrets"))
            setMode(Path("$root/secrets"), 0b111_101_101)
            val e = assertFailsWith<SecretException> { FileSecretStore("$root/secrets").save("token", "t") }
            assertContains(e.message!!, "chmod 700")
        }
}

class OwnerOnlyTest {
    @Test
    fun a_new_directory_is_created_owner_only_and_kept() =
        home { root ->
            ensureOwnerOnlyDir("$root/run")
            ensureOwnerOnlyDir("$root/run")
            if (posix) assertEquals(0b111_000_000, mode(Path("$root/run"))!! and 0b111_111_111)
        }

    @Test
    fun a_link_or_a_missing_parent_is_refused() =
        home { root ->
            if (!posix) return@home
            createDir(Path("$root/real"))
            setMode(Path("$root/real"), 0b111_000_000)
            symlink(Path("$root/real"), Path("$root/run"))
            assertContains(assertFailsWith<OwnerOnlyException> { ensureOwnerOnlyDir("$root/run") }.message!!, "not a link")
            assertFailsWith<OwnerOnlyException> { ensureOwnerOnlyDir("$root/missing/run") }
        }

    @Test
    fun a_file_is_restricted_to_its_owner() =
        home { root ->
            if (!posix) return@home
            WriteOptions(mode = 0b110_100_100).write(Path("$root/f"), byteArrayOf(1))
            restrictToOwner("$root/f")
            assertEquals(0b110_000_000, mode(Path("$root/f"))!! and 0b111_111_111)
        }
}

class FallbackSecretStoreTest {
    private class Broken : SecretStore {
        override val description = "a broken keychain"

        override fun save(
            name: String,
            value: String,
        ) = throw SecretException("locked")

        override fun load(name: String): String? = throw SecretException("locked")

        override fun delete(name: String): Boolean = throw SecretException("locked")
    }

    @Test
    fun a_failing_keychain_falls_back_to_files_with_a_warning() =
        home { root ->
            val warnings = mutableListOf<String>()
            val store = FallbackSecretStore(Broken(), FileSecretStore("$root/secrets"), warnings::add)
            store.save("token", "secret-value")
            assertEquals("secret-value", store.load("token"))
            assertTrue(store.delete("token"))
            assertEquals(3, warnings.size)
            assertTrue(warnings.none { "secret-value" in it }, "a warning never carries the secret")
            assertContains(warnings.first(), "a broken keychain")
        }
}

class SystemSecretStoreTest {
    /**
     * Writes to the real keychain, so it runs only with AULO_KEYCHAIN_TEST set: the macOS
     * Keychain, Credential Manager, the Secret Service, or the file fallback where there is none.
     */
    @OptIn(ExperimentalForeignApi::class)
    @Test
    fun the_system_store_round_trips() {
        if (getenv("AULO_KEYCHAIN_TEST")?.toKString().isNullOrEmpty()) return
        home { root ->
            val warnings = mutableListOf<String>()
            val store = systemSecretStore("aulo-test-${Random.nextLong().toULong()}", "$root/secrets", warnings::add)
            try {
                assertNull(store.load("token"))
                store.save("token", "one")
                store.save("token", "two")
                assertEquals("two", store.load("token"))
            } finally {
                store.delete("token")
            }
            assertNull(store.load("token"))
            println("system secret store: ${store.description}; warnings: $warnings")
        }
    }
}

private fun home(block: (String) -> Unit) = TempDir.create("aulo-secrets").use { block(it.path.toString()) }
