package aulo.voice.speak

// Spoken forms of numbers and units for English and Russian. Deliberately small: digits stay digits (every TTS engine
// reads them), and only what engines get wrong is rewritten: decimal and thousands separators, `%` and a table of
// common units, with Russian plural forms.

/** A unit's symbols, its English singular and plural, and its Russian forms for 1, 2–4 and 5+. */
private class SpokenUnit(
    val symbols: List<String>,
    val en: List<String>,
    val ru: List<String>,
) {
    fun forms(lang: Language): List<String> =
        when (lang) {
            Language.EN -> en
            Language.RU -> ru
        }
}

private fun unit(
    symbols: String,
    en: String,
    enPlural: String,
    ru1: String,
    ru2: String,
    ru5: String,
) = SpokenUnit(symbols.split('|'), listOf(en, enPlural), listOf(ru1, ru2, ru5))

// The order matters only where one symbol is a prefix of another; the word-end check keeps "m" from matching "ms".
private val UNITS =
    listOf(
        unit("%", "percent", "percent", "процент", "процента", "процентов"),
        unit("°C", "degree Celsius", "degrees Celsius", "градус Цельсия", "градуса Цельсия", "градусов Цельсия"),
        unit("°F", "degree Fahrenheit", "degrees Fahrenheit", "градус Фаренгейта", "градуса Фаренгейта", "градусов Фаренгейта"),
        unit("km|км", "kilometer", "kilometers", "километр", "километра", "километров"),
        unit("m|м", "meter", "meters", "метр", "метра", "метров"),
        unit("cm|см", "centimeter", "centimeters", "сантиметр", "сантиметра", "сантиметров"),
        unit("mm|мм", "millimeter", "millimeters", "миллиметр", "миллиметра", "миллиметров"),
        unit("kg|кг", "kilogram", "kilograms", "килограмм", "килограмма", "килограммов"),
        unit("g|г", "gram", "grams", "грамм", "грамма", "граммов"),
        unit("mg|мг", "milligram", "milligrams", "миллиграмм", "миллиграмма", "миллиграммов"),
        unit("l|л", "liter", "liters", "литр", "литра", "литров"),
        unit("ml|мл", "milliliter", "milliliters", "миллилитр", "миллилитра", "миллилитров"),
        unit("ms|мс", "millisecond", "milliseconds", "миллисекунда", "миллисекунды", "миллисекунд"),
        unit("s|сек", "second", "seconds", "секунда", "секунды", "секунд"),
        unit("min|мин", "minute", "minutes", "минута", "минуты", "минут"),
        unit("h|ч", "hour", "hours", "час", "часа", "часов"),
        unit("KB|КБ", "kilobyte", "kilobytes", "килобайт", "килобайта", "килобайт"),
        unit("MB|МБ", "megabyte", "megabytes", "мегабайт", "мегабайта", "мегабайт"),
        unit("GB|ГБ", "gigabyte", "gigabytes", "гигабайт", "гигабайта", "гигабайт"),
        unit("TB|ТБ", "terabyte", "terabytes", "терабайт", "терабайта", "терабайт"),
        unit("Hz|Гц", "hertz", "hertz", "герц", "герц", "герц"),
        unit("kHz|кГц", "kilohertz", "kilohertz", "килогерц", "килогерц", "килогерц"),
        unit("MHz|МГц", "megahertz", "megahertz", "мегагерц", "мегагерц", "мегагерц"),
        unit("GHz|ГГц", "gigahertz", "gigahertz", "гигагерц", "гигагерц", "гигагерц"),
    )

private const val THOUSANDS_GROUP = 3
private const val HUNDRED = 100
private const val TEN = 10

/** An integer or decimal read from text, separators already removed. */
private class Quantity(
    val int: String,
    val frac: String?,
) {
    /** Index of the unit form: English singular only for exactly 1; Russian by the last digits, decimals genitive. */
    fun form(lang: Language): Int {
        if (lang == Language.EN) return if (frac == null && int.trimStart('0') == "1") 0 else 1
        if (frac != null) return 1
        val n = int.takeLast(2).toInt()
        return when {
            n % TEN == 1 && n % HUNDRED != 11 -> 0
            n % TEN in 2..4 && n % HUNDRED !in 12..14 -> 1
            else -> 2
        }
    }

    fun write(
        out: StringBuilder,
        lang: Language,
    ) {
        out.append(int)
        val frac = frac ?: return
        when (lang) {
            // English engines read "14" as fourteen, so decimals go digit by digit.
            Language.EN -> {
                out.append(" point")
                frac.forEach { out.append(' ').append(it) }
            }

            Language.RU -> {
                out.append(" запятая ").append(frac)
            }
        }
    }

    companion object {
        /**
         * `1,234.5` in English, `1234,5` in Russian (where `.` is read as the decimal mark too). Anything else, such
         * as a date, a time, a version or an address, is not a number and stays as written.
         */
        fun parse(
            run: String,
            lang: Language,
        ): Quantity? {
            val sep =
                when (lang) {
                    Language.EN -> run.indexOf('.')
                    Language.RU -> run.indexOfAny(charArrayOf(',', '.'))
                }
            val head = if (sep < 0) run else run.substring(0, sep)
            val frac = if (sep < 0) null else run.substring(sep + 1)
            if (frac != null && !frac.allDigits()) return null
            val int =
                if (lang == Language.EN && ',' in head) {
                    val groups = head.split(',')
                    val grouped =
                        groups[0].length <= THOUSANDS_GROUP &&
                            groups[0].allDigits() &&
                            groups.drop(1).all { it.length == THOUSANDS_GROUP && it.allDigits() }
                    if (!grouped) return null
                    head.replace(",", "")
                } else {
                    head.takeIf { it.allDigits() } ?: return null
                }
            return Quantity(int, frac)
        }
    }
}

private fun String.allDigits(): Boolean = isNotEmpty() && all { it in '0'..'9' }

private const val NUMBER_PUNCTUATION = ".,:/-"

/**
 * A unit at [at], right after a number or after one space, with the number of characters it takes. Single-letter
 * symbols must touch the number: "5 m" is a unit, "I have 2 s" is not worth the risk.
 */
private fun unitAfter(
    text: String,
    at: Int,
): Pair<SpokenUnit, Int>? {
    val gap = if (text.getOrNull(at) == ' ') 1 else 0
    val start = at + gap
    for (unit in UNITS) {
        for (symbol in unit.symbols) {
            if (!text.startsWith(symbol, start)) continue
            val next = text.getOrNull(start + symbol.length)
            // A following `/` means a compound unit (km/h) that is not in the table.
            val wordEnds = next == null || (next != '/' && !next.isLetterOrDigit())
            val gapOk = gap == 0 || symbol.length > 1 || symbol == "%"
            if (wordEnds && gapOk) return unit to gap + symbol.length
        }
    }
    return null
}

internal fun expand(
    text: String,
    lang: Language,
): String {
    val out = StringBuilder(text.length)
    var i = 0
    var prevAlnum = false
    while (i < text.length) {
        val c = text[i]
        // Digits inside words ("v2", "utf8") are identifiers, not quantities.
        if (c !in '0'..'9' || prevAlnum) {
            out.append(c)
            prevAlnum = c.isLetterOrDigit()
            i++
            continue
        }
        var end = i
        while (end < text.length && (text[end] in '0'..'9' || text[end] in NUMBER_PUNCTUATION)) end++
        // Trailing punctuation belongs to the sentence, not to the number.
        while (text[end - 1] in NUMBER_PUNCTUATION) end--
        val run = text.substring(i, end)
        i = end
        prevAlnum = false
        val number = Quantity.parse(run, lang)
        if (number == null) {
            out.append(run)
            continue
        }
        number.write(out, lang)
        val (unit, used) = unitAfter(text, i) ?: continue
        val forms = unit.forms(lang)
        out.append(' ').append(forms[number.form(lang).coerceAtMost(forms.size - 1)])
        i += used
    }
    return out.toString()
}
