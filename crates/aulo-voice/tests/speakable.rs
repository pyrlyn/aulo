//! Golden tests: one snapshot per language over the same kinds of input, so a
//! rule change shows up as a reviewable diff of spoken text.

use aulo_voice::{
    Language, MAX_INPUT_BYTES, MAX_SPOKEN_CHARS, SpeakOptions, detect_language, speakable,
};
use insta::assert_snapshot;

const EN: &[&str] = &[
    "# Plan\n\nI **fixed** the _build_ and ~~broke~~ nothing.",
    "- first item\n- second item\n\n1. step one\n2. step two",
    "Run this:\n\n```sh\ncargo test --workspace\n```\n\nThen look at the result.",
    "```\none\n```\n\n```\ntwo\n```",
    "    indented code\n\nDone",
    "| a | b |\n|---|---|\n| 1 | 2 |\n\nThat is the table.",
    "See [the docs](https://docs.example.com/guide?x=1#top) or <https://www.rust-lang.org/learn>.",
    "Open https://github.com/rust-lang/rust/issues/1, then https://[bad.",
    "It took 3.14 seconds, 42% of 1,234 users used 5 km, 2.5 GB and 1 MB at 100MHz.",
    "Version 1.2.3 shipped on 2024-05-01 at 12:30 from 192.168.0.1; utf8 and v2 stay.",
    "He said <b>hi</b>\u{202E}there\u{200B}\u{7} <!-- note --> and left.",
    "",
];

const RU: &[&str] = &[
    "# План\n\nЯ **починил** сборку и ничего не сломал.",
    "- первый пункт\n- второй пункт",
    "Запусти:\n\n```sh\ncargo test\n```\n\nПотом посмотри на результат.",
    "Смотри [документацию](https://docs.example.ru/guide) или https://www.example.ru/page.",
    "Заняло 3,14 секунды, 42% из 1234 пользователей, 1 км, 2 км, 5 км, 21 км.",
    "Скачано 2,5 ГБ: 1 MB, 2 MB, 5 MB, 11 MB, 22 GB, 3 min, 1 час, 100MHz, 21°C.",
    "Версия 1.2.3 вышла 01.05.2024 в 12:30.",
    "Скачано 2,5 ГБ за 3 сек, 1 кг, 2 кг, 5 кг, 20 мин, 7 см, 1 м, 25 км/ч.",
];

fn render(cases: &[&str], language: Language) -> String {
    cases
        .iter()
        .map(|case| {
            format!(
                "{case:?}\n=> {:?}\n",
                speakable(case, SpeakOptions::new(language))
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn english_golden() {
    assert_snapshot!(render(EN, Language::En));
}

#[test]
fn russian_golden() {
    assert_snapshot!(render(RU, Language::Ru));
}

#[test]
fn input_is_capped_and_long_output_goes_to_the_screen() {
    let en = SpeakOptions::new(Language::En);
    let long = "Word. ".repeat(MAX_SPOKEN_CHARS);
    let spoken = speakable(&long, en);
    assert!(spoken.ends_with(Language::En.on_screen()), "{spoken}");
    assert!(spoken.chars().count() <= MAX_SPOKEN_CHARS + Language::En.on_screen().len());

    // Far more input than the cap, ending in a multi-byte character at the cut.
    let huge = "я".repeat(MAX_INPUT_BYTES);
    let spoken = speakable(&huge, SpeakOptions::new(Language::Ru));
    assert!(spoken.ends_with(Language::Ru.on_screen()), "{spoken}");

    let tiny = SpeakOptions {
        max_input_bytes: 5,
        ..en
    };
    assert_eq!(speakable("Hello world", tiny), "Hello.");
}

#[test]
fn nothing_speakable_gives_an_empty_string() {
    let en = SpeakOptions::new(Language::En);
    for input in [
        "",
        "   \n\t",
        "<!-- only a comment -->",
        "\u{202E}\u{200B}",
        "---",
    ] {
        assert_eq!(speakable(input, en), "", "{input:?}");
    }
}

#[test]
fn language_helpers() {
    assert_eq!(Language::from_tag("ru-RU"), Some(Language::Ru));
    assert_eq!(Language::from_tag("EN_us"), Some(Language::En));
    assert_eq!(Language::from_tag("de"), None);
    assert_eq!(detect_language("Hello there"), Some(Language::En));
    assert_eq!(detect_language("Привет, мир"), Some(Language::Ru));
    assert_eq!(detect_language("Запусти cargo test"), Some(Language::Ru));
    assert_eq!(detect_language("12 + 3 = 15"), None);
}
