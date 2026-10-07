use std::process::Command;

const AULO: &str = env!("CARGO_BIN_EXE_aulo");

#[test]
fn command_output_fixtures() {
    trycmd::TestCases::new().case("tests/cmd/*.toml");
}

#[test]
fn version_flag_names_the_binary() {
    let out = Command::new(AULO).arg("--version").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("aulo "), "{text}");
}

#[test]
fn config_default_prints_the_embedded_file_byte_for_byte() {
    let out = Command::new(AULO)
        .args(["config", "default"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        aulo_config::DEFAULT_TOML
    );
}

#[test]
fn no_subcommand_prints_help_and_fails() {
    let out = Command::new(AULO).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("Usage: aulo")
    );
}
