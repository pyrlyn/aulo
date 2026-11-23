package aulo.buildlogic

import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals

class ConanTest {
    @Test
    fun onlyTheRepositorysRecipesMayBeBuiltWhereNothingElseMay() {
        assertEquals(listOf("--build=never"), buildOnly(emptyList()))
        assertEquals(listOf("--build=missing:a/*", "--build=missing:b/*"), buildOnly(listOf("a", "b")))
    }

    @Test
    fun aRecipeIsADirectoryWithAConanfile() {
        val profiles = Files.createTempDirectory("conan").toFile()
        try {
            assertEquals(emptyList(), localRecipes(profiles))
            val recipes = profiles.resolve("recipes")
            recipes.resolve("zeta").mkdirs()
            recipes.resolve("zeta/conanfile.py").writeText("")
            recipes.resolve("alpha").mkdirs()
            recipes.resolve("alpha/conanfile.py").writeText("")
            recipes.resolve("notes").mkdirs()
            recipes.resolve("README.md").writeText("")
            assertEquals(listOf("alpha", "zeta"), localRecipes(profiles).map { it.name })
        } finally {
            profiles.deleteRecursively()
        }
    }
}
