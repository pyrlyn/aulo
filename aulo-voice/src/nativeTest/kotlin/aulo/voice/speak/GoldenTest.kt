package aulo.voice.speak

import fs.atomic
import fs.readText
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.io.files.Path
import platform.posix.getenv
import kotlin.test.Test
import kotlin.test.assertEquals

/** One snapshot per language over the same kinds of input, so a rule change shows up as a reviewable diff. */
class GoldenTest {
    private val en =
        listOf(
            "# Plan\n\nI **fixed** the _build_ and ~~broke~~ nothing.",
            "- first item\n- second item\n\n1. step one\n2. step two",
            "Run this:\n\n```sh\ncargo test --workspace\n```\n\nThen look at the result.",
            "```\none\n```\n\n```\ntwo\n```",
            "    indented code\n\nDone",
            "| a | b |\n|---|---|\n| 1 | 2 |\n\nThat is the table.",
            "See [the docs](https://docs.example.com/guide?x=1#top) or <https://www.rust-lang.org/learn>.",
            "Open https://github.com/rust-lang/rust/issues/1, then https://[bad.",
            "Log in at https://admin:hunter2@Example.COM:8443/x and ![logo](https://example.com/a.png) here.",
            "It took 3.14 seconds, 42% of 1,234 users used 5 km, 2.5 GB and 1 MB at 100MHz.",
            "Version 1.2.3 shipped on 2024-05-01 at 12:30 from 192.168.0.1; utf8 and v2 stay.",
            "He said <b>hi</b>${Char(0x202E)}there${Char(0x200B)}\u0007 <!-- note --> and left.",
            "Tom &amp; Jerry \\*escaped\\* and `a  b` code.\n\n> quoted\n> text\n\n---\n\nSetext\n======\n\n- [x] done task",
            "&#x202E;evil &#0; &copy; &#128512; &bogus; &#xD800; \\[x\\] stay",
            "Costs \$5 and \$10. Line one\nline two  \nline three",
            "",
        )

    private val ru =
        listOf(
            "# План\n\nЯ **починил** сборку и ничего не сломал.",
            "- первый пункт\n- второй пункт",
            "Запусти:\n\n```sh\ncargo test\n```\n\nПотом посмотри на результат.",
            "Смотри [документацию](https://docs.example.ru/guide) или https://www.example.ru/page.",
            "Заняло 3,14 секунды, 42% из 1234 пользователей, 1 км, 2 км, 5 км, 21 км.",
            "Скачано 2,5 ГБ: 1 MB, 2 MB, 5 MB, 11 MB, 22 GB, 3 min, 1 час, 100MHz, 21°C.",
            "Версия 1.2.3 вышла 01.05.2024 в 12:30.",
            "Скачано 2,5 ГБ за 3 сек, 1 кг, 2 кг, 5 кг, 20 мин, 7 см, 1 м, 25 км/ч.",
            "Открой https://пример.рф/путь и 112 км, 111 км, 104 км.",
        )

    @Test fun english() = check("speakable_en.txt", render(en, Language.EN))

    @Test fun russian() = check("speakable_ru.txt", render(ru, Language.RU))

    private fun render(
        cases: List<String>,
        language: Language,
    ): String = cases.joinToString("\n") { "${quote(it)}\n=> ${quote(speakable(it, SpeakOptions(language)))}\n" }

    private fun quote(s: String): String =
        buildString {
            append('"')
            for (c in s) {
                when {
                    c == '"' || c == '\\' -> append('\\').append(c)
                    c == '\n' -> append("\\n")
                    c == '\t' -> append("\\t")
                    c.isISOControl() || c.category == CharCategory.FORMAT -> append("\\u{").append(c.code.toString(HEX)).append('}')
                    else -> append(c)
                }
            }
            append('"')
        }

    @OptIn(ExperimentalForeignApi::class)
    private fun env(name: String) = getenv(name)?.toKString().orEmpty()

    private fun check(
        name: String,
        actual: String,
    ) {
        val dir = env("AULO_VOICE_SNAPSHOTS").ifEmpty { error("AULO_VOICE_SNAPSHOTS is set by the Gradle test task") }
        val file = Path(dir, name)
        if (env("AULO_UPDATE_FIXTURES") == "1") {
            atomic(file, actual.encodeToByteArray())
            return
        }
        assertEquals(readText(file), actual, "$name is stale: rerun with AULO_UPDATE_FIXTURES=1 and review the diff")
    }

    private companion object {
        const val HEX = 16
    }
}
