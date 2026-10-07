// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use aulo_plugin::manifest::{
    Audio, Entry, Kind, MAX_MANIFEST_BYTES, ManifestError, PluginManifest,
};

/// A TOML basic string for any value; JSON string escapes are valid TOML.
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

const FILE: &str = "plugins/demo/aulo-plugin.toml";

fn parse(text: &str) -> Result<PluginManifest, ManifestError> {
    PluginManifest::from_toml(text, Path::new(FILE))
}

fn message(text: &str) -> String {
    parse(text).unwrap_err().to_string()
}

/// Asserts the rejection names the file and the offending field.
fn rejected(text: &str, needle: &str) {
    let msg = message(text);
    assert!(msg.contains(FILE), "{msg}");
    assert!(msg.contains(needle), "expected {needle:?} in {msg}");
}

const TTS: &str = r#"
id = "piper-tts"
version = "1.4.0-beta.1"
kinds = ["tts"]

[entry]
type = "grpc-process"
command = "bin/piper-plugin"
args = ["--socket-dir", "run"]

[capabilities]
net = ["huggingface.co", "*.hf.co", "127.0.0.1", "::1"]
fs_read = ["/usr/share/piper", 'C:\piper\voices']
fs_write = ["/var/cache/piper"]
exec = false
audio = ["playback"]
"#;

#[test]
fn valid_grpc_plugin_parses_with_all_fields() {
    let m = parse(TTS).unwrap();
    assert_eq!(m.id, "piper-tts");
    assert_eq!(m.version.to_string(), "1.4.0-beta.1");
    assert!(m.kinds.contains(&Kind::Tts));
    assert!(matches!(m.entry, Some(Entry::GrpcProcess(ref p)) if p.args.len() == 2));
    assert_eq!(m.capabilities.net.len(), 4);
    assert!(m.capabilities.audio.contains(&Audio::Playback));
}

#[test]
fn minimal_skills_plugin_needs_no_entry_or_capabilities() {
    let m = parse("id = \"notes\"\nversion = \"0.1.0\"\nkinds = [\"skills\"]\n").unwrap();
    assert!(m.entry.is_none());
    assert_eq!(m.capabilities, Default::default());
}

#[test]
fn tools_accept_mcp_or_wasm_entries() {
    let head = "id = \"t\"\nversion = \"1.0.0\"\nkinds = [\"tools\", \"hooks\"]\n[entry]\n";
    parse(&format!(
        "{head}type = \"mcp-process\"\ncommand = \"node\"\n"
    ))
    .unwrap();
    parse(&format!(
        "{head}type = \"wasm\"\nmodule = \"plugin.wasm\"\n"
    ))
    .unwrap();
}

#[test]
fn hooks_only_plugin_may_use_wasm() {
    let wasm = "id = \"h\"\nversion = \"1.0.0\"\nkinds = [\"hooks\"]\n[entry]\ntype = \"wasm\"\nmodule = \"h.wasm\"\n";
    parse(wasm).unwrap();
}

#[test]
fn unknown_top_level_key_is_rejected_naming_the_file() {
    rejected(&format!("{TTS}\nsurprise = 1\n"), "surprise");
}

#[test]
fn unknown_nested_keys_are_rejected() {
    rejected(
        &TTS.replace("exec = false", "exec = false\nroot = true"),
        "root",
    );
    rejected(
        &TTS.replace("command =", "shell = true\ncommand ="),
        "shell",
    );
}

#[test]
fn unknown_entry_type_is_rejected() {
    rejected(&TTS.replace("grpc-process", "dylib"), "dylib");
}

#[test]
fn invalid_ids_are_rejected() {
    let too_long = "a".repeat(33);
    for id in [
        "",
        "Upper",
        "1abc",
        "-a",
        "a-",
        "a--b",
        "snake_case",
        "has space",
        "../x",
        "naïve",
        too_long.as_str(),
    ] {
        rejected(&TTS.replace("piper-tts", id), "id ");
    }
    parse(&TTS.replace("piper-tts", &"a".repeat(32))).unwrap();
}

#[test]
fn version_must_be_semver() {
    for v in ["1", "1.2", "v1.2.3", "01.2.3", "latest", ""] {
        let msg = message(&TTS.replace("1.4.0-beta.1", v));
        assert!(msg.contains(FILE), "{msg}");
    }
}

#[test]
fn kinds_must_be_present_non_empty_and_known() {
    rejected(&TTS.replace("kinds = [\"tts\"]", "kinds = []"), "kinds");
    rejected(&TTS.replace("kinds = [\"tts\"]\n", ""), "kinds");
    rejected(&TTS.replace("\"tts\"", "\"video\""), "video");
}

#[test]
fn entry_must_match_the_declared_kinds() {
    let grpc = |kinds: &str| TTS.replace("[\"tts\"]", kinds);
    // Streaming kind without an entry, or with an MCP entry.
    rejected(&TTS.replace("grpc-process", "mcp-process"), "grpc-process");
    let no_entry = "id = \"x\"\nversion = \"1.0.0\"\nkinds = [\"stt\"]\n";
    rejected(no_entry, "grpc-process");
    // Tools over gRPC, or without any entry.
    rejected(&grpc("[\"tools\"]"), "mcp-process");
    rejected(
        "id = \"x\"\nversion = \"1.0.0\"\nkinds = [\"tools\"]\n",
        "mcp-process",
    );
    // One plugin cannot need two ABIs.
    rejected(&grpc("[\"tools\", \"tts\"]"), "split");
    // Skills never run a process.
    rejected(&grpc("[\"skills\"]"), "skills");
    // A process entry on hooks is not allowed either.
    rejected(&grpc("[\"hooks\"]"), "hooks");
}

#[test]
fn entry_paths_must_stay_inside_the_package() {
    for cmd in [
        "/usr/bin/evil",
        "../evil",
        "bin/../../evil",
        "C:\\evil.exe",
        "",
        "a\nb",
    ] {
        rejected(
            &TTS.replace("\"bin/piper-plugin\"", &q(cmd)),
            "entry.command",
        );
    }
    let wasm = |m: &str| {
        format!(
            "id = \"x\"\nversion = \"1.0.0\"\nkinds = [\"tools\"]\n[entry]\ntype = \"wasm\"\nmodule = {m:?}\n"
        )
    };
    rejected(&wasm("/abs/x.wasm"), "entry.module");
    rejected(&wasm("../x.wasm"), "entry.module");
    rejected(&wasm(""), "entry.module");
}

#[test]
fn net_hosts_must_be_well_formed() {
    for host in [
        "",
        "*",
        "*.com",
        "https://example.com",
        "example.com:443",
        "example.com/path",
        "Example.com",
        "exa mple.com",
        "-bad.com",
        "bad-.com",
        "a..b",
        "999.1.1.1.1",
        "[::1]",
        "*.*.example.com",
    ] {
        rejected(&TTS.replace("huggingface.co", host), "capabilities.net");
    }
    let many = vec!["\"a.com\""; 65].join(", ");
    rejected(
        &TTS.replace(
            "[\"huggingface.co\", \"*.hf.co\", \"127.0.0.1\", \"::1\"]",
            &format!("[{many}]"),
        ),
        "capabilities.net",
    );
}

#[test]
fn fs_roots_must_be_absolute_and_specific() {
    for root in [
        "relative/dir",
        "~/x",
        "/",
        "C:\\",
        "/a/../b",
        "/a/./b",
        "",
        "/a\0b",
    ] {
        rejected(
            &TTS.replace("\"/usr/share/piper\"", &q(root)),
            "capabilities.fs_read",
        );
        rejected(
            &TTS.replace("\"/var/cache/piper\"", &q(root)),
            "capabilities.fs_write",
        );
    }
}

#[test]
fn oversized_text_is_rejected_before_parsing() {
    let big = format!("{TTS}\n# {}\n", "x".repeat(MAX_MANIFEST_BYTES as usize));
    assert!(matches!(parse(&big), Err(ManifestError::TooLarge { .. })));
    rejected(&big, "larger than");
}

#[test]
fn load_reads_a_file_and_names_it_on_every_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aulo-plugin.toml");

    let missing = PluginManifest::load(&path).unwrap_err();
    assert!(matches!(missing, ManifestError::Read { .. }));
    assert!(missing.to_string().contains("aulo-plugin.toml"));

    std::fs::write(&path, TTS).unwrap();
    assert_eq!(PluginManifest::load(&path).unwrap().id, "piper-tts");

    std::fs::write(&path, vec![b'#'; MAX_MANIFEST_BYTES as usize + 1]).unwrap();
    let big = PluginManifest::load(&path).unwrap_err();
    assert!(matches!(big, ManifestError::TooLarge { .. }));
    assert!(big.to_string().contains("aulo-plugin.toml"));

    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(matches!(
        PluginManifest::load(&path),
        Err(ManifestError::Read { .. })
    ));

    std::fs::write(&path, "id = ").unwrap();
    let bad = PluginManifest::load(&path).unwrap_err();
    assert!(matches!(bad, ManifestError::Parse { .. }));
    assert!(bad.to_string().contains("aulo-plugin.toml"));
}

#[test]
fn error_messages_escape_and_truncate_hostile_values() {
    let msg = message(&TTS.replace("piper-tts", &format!("{}\\u001b[31m", "z".repeat(100))));
    assert!(!msg.contains('\u{1b}'));
    assert!(msg.len() < 400, "{msg}");
}
