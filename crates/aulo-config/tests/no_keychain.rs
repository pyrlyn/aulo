//! No test may reach the real OS keychain. Production code may; tests use
//! [`aulo_config::MemoryStore`].

#[test]
fn no_test_reads_the_real_keychain() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut forbidden = keychain_secret::guard::KEYCHAIN_CALLS.to_vec();
    forbidden.push("system_store(");
    let found = keychain_secret::guard::violations(&root, &forbidden);
    assert!(
        found.is_empty(),
        "a test touches the real keychain:\n{}",
        found.join("\n")
    );
}
