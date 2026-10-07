//! Spoken forms of numbers and units for English and Russian.
//!
//! Deliberately small: digits stay digits (every TTS engine reads them), and
//! only what engines get wrong is rewritten: decimal separators, thousands
//! separators, `%` and a table of common units, with Russian plural forms.

use crate::speakable::Language;

/// `(symbols separated by "|", English [singular, plural], Russian [1, 2-4, 5+])`.
type Unit = (&'static str, [&'static str; 2], [&'static str; 3]);

// One row per line reads better than rustfmt's three-line expansion.
#[rustfmt::skip]
const UNITS: &[Unit] = &[
    ("%", ["percent"; 2], ["процент", "процента", "процентов"]),
    ("°C", ["degree Celsius", "degrees Celsius"], ["градус Цельсия", "градуса Цельсия", "градусов Цельсия"]),
    ("°F", ["degree Fahrenheit", "degrees Fahrenheit"], ["градус Фаренгейта", "градуса Фаренгейта", "градусов Фаренгейта"]),
    ("km|км", ["kilometer", "kilometers"], ["километр", "километра", "километров"]),
    ("m|м", ["meter", "meters"], ["метр", "метра", "метров"]),
    ("cm|см", ["centimeter", "centimeters"], ["сантиметр", "сантиметра", "сантиметров"]),
    ("mm|мм", ["millimeter", "millimeters"], ["миллиметр", "миллиметра", "миллиметров"]),
    ("kg|кг", ["kilogram", "kilograms"], ["килограмм", "килограмма", "килограммов"]),
    ("g|г", ["gram", "grams"], ["грамм", "грамма", "граммов"]),
    ("mg|мг", ["milligram", "milligrams"], ["миллиграмм", "миллиграмма", "миллиграммов"]),
    ("l|л", ["liter", "liters"], ["литр", "литра", "литров"]),
    ("ml|мл", ["milliliter", "milliliters"], ["миллилитр", "миллилитра", "миллилитров"]),
    ("ms|мс", ["millisecond", "milliseconds"], ["миллисекунда", "миллисекунды", "миллисекунд"]),
    ("s|сек", ["second", "seconds"], ["секунда", "секунды", "секунд"]),
    ("min|мин", ["minute", "minutes"], ["минута", "минуты", "минут"]),
    ("h|ч", ["hour", "hours"], ["час", "часа", "часов"]),
    ("KB|КБ", ["kilobyte", "kilobytes"], ["килобайт", "килобайта", "килобайт"]),
    ("MB|МБ", ["megabyte", "megabytes"], ["мегабайт", "мегабайта", "мегабайт"]),
    ("GB|ГБ", ["gigabyte", "gigabytes"], ["гигабайт", "гигабайта", "гигабайт"]),
    ("TB|ТБ", ["terabyte", "terabytes"], ["терабайт", "терабайта", "терабайт"]),
    ("Hz|Гц", ["hertz"; 2], ["герц"; 3]),
    ("kHz|кГц", ["kilohertz"; 2], ["килогерц"; 3]),
    ("MHz|МГц", ["megahertz"; 2], ["мегагерц"; 3]),
    ("GHz|ГГц", ["gigahertz"; 2], ["гигагерц"; 3]),
];

/// An integer or decimal read from text, separators already removed.
struct Number<'a> {
    int: String,
    frac: Option<&'a str>,
}

impl<'a> Number<'a> {
    /// `1,234.5` in English, `1234,5` in Russian (where `.` is read as the
    /// decimal mark too). Anything else, such as a date, a time, a version or
    /// an address, is not a number and stays as written.
    fn parse(run: &'a str, lang: Language) -> Option<Self> {
        let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        let (head, frac) = match lang {
            Language::En => run
                .split_once('.')
                .map_or((run, None), |(h, f)| (h, Some(f))),
            Language::Ru => run
                .split_once([',', '.'])
                .map_or((run, None), |(h, f)| (h, Some(f))),
        };
        if frac.is_some_and(|f| !all_digits(f)) {
            return None;
        }
        let int = if lang == Language::En && head.contains(',') {
            let mut groups = head.split(',');
            let first = groups.next()?;
            let thousands_ok = first.len() <= 3
                && all_digits(first)
                && groups.all(|g| g.len() == 3 && all_digits(g));
            thousands_ok.then(|| head.replace(',', ""))?
        } else {
            all_digits(head).then(|| head.to_owned())?
        };
        Some(Self { int, frac })
    }

    /// Index of the unit form: English singular only for exactly 1; Russian
    /// by the last digits, with decimals taking the genitive singular.
    fn form(&self, lang: Language) -> usize {
        match lang {
            Language::En => {
                usize::from(self.frac.is_some() || self.int.trim_start_matches('0') != "1")
            }
            Language::Ru if self.frac.is_some() => 1,
            Language::Ru => {
                let tail = &self.int[self.int.len().saturating_sub(2)..];
                let n: u32 = tail.parse().unwrap_or(0);
                match (n % 10, n % 100) {
                    (1, r) if r != 11 => 0,
                    (2..=4, r) if !(12..=14).contains(&r) => 1,
                    _ => 2,
                }
            }
        }
    }

    fn write(&self, out: &mut String, lang: Language) {
        out.push_str(&self.int);
        let Some(frac) = self.frac else { return };
        match lang {
            // English engines read "14" as fourteen, so decimals go digit by digit.
            Language::En => {
                out.push_str(" point");
                for digit in frac.chars() {
                    out.push(' ');
                    out.push(digit);
                }
            }
            Language::Ru => {
                out.push_str(" запятая ");
                out.push_str(frac);
            }
        }
    }
}

/// A unit right after a number, or after one space. Single-letter symbols
/// must touch the number: "5 m" is a unit, "I have 2 s" is not worth the risk.
fn unit_after(rest: &str) -> Option<(&'static Unit, usize)> {
    let (tail, gap) = rest.strip_prefix(' ').map_or((rest, 0), |t| (t, 1));
    UNITS.iter().find_map(|unit| {
        unit.0.split('|').find_map(|symbol| {
            let after = tail.strip_prefix(symbol)?;
            // A following `/` means a compound unit (km/h) that is not in the table.
            let word_ends =
                !after.starts_with('/') && !after.chars().next().is_some_and(char::is_alphanumeric);
            let gap_ok = gap == 0 || symbol.chars().count() > 1 || symbol == "%";
            (word_ends && gap_ok).then_some((unit, gap + symbol.len()))
        })
    })
}

pub(crate) fn expand(text: &str, lang: Language) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut prev_alnum = false;
    while let Some(c) = rest.chars().next() {
        // Digits inside words ("v2", "utf8") are identifiers, not quantities.
        if !c.is_ascii_digit() || prev_alnum {
            out.push(c);
            prev_alnum = c.is_alphanumeric();
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let run_len = rest
            .find(|ch: char| !(ch.is_ascii_digit() || ".,:/-".contains(ch)))
            .unwrap_or(rest.len());
        // Trailing punctuation belongs to the sentence, not to the number.
        let run = rest[..run_len].trim_end_matches(['.', ',', ':', '/', '-']);
        rest = &rest[run.len()..];
        prev_alnum = false;
        let Some(number) = Number::parse(run, lang) else {
            out.push_str(run);
            continue;
        };
        number.write(&mut out, lang);
        if let Some((unit, used)) = unit_after(rest) {
            out.push(' ');
            let forms: &[&str] = match lang {
                Language::En => &unit.1,
                Language::Ru => &unit.2,
            };
            out.push_str(forms[number.form(lang).min(forms.len() - 1)]);
            rest = &rest[used..];
        }
    }
    out
}
