//! Enforces the crate roles of spec §4.2 on the real workspace by parsing
//! `cargo metadata`, so the rules cannot drift from the `Cargo.toml` files.
//!
//! The rules are pure functions over a [`Graph`] so that tests can feed them a
//! synthetic graph and prove each one fails on a violation. The role table
//! lists planned crates too: a crate that is not in the workspace yet simply
//! never matches, while a workspace crate missing from the table is an error,
//! so every new crate has to be given a role.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

/// Crate name -> names of its direct normal (non-dev, non-build) dependencies,
/// workspace crates and external ones alike. Optional and target-specific
/// dependencies count: a feature-gated heavy dependency is still a dependency.
type Graph = BTreeMap<String, BTreeSet<String>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Contract,
    Domain,
    Adapter,
    Assembly,
    Surface,
    Testkit,
}

/// Spec §4.2, one row per crate.
const ROLES: &[(&str, Role)] = &[
    ("aulo-types", Role::Contract),
    ("aulo-proto", Role::Contract),
    ("aulo-speech", Role::Contract),
    ("aulo-plugin-sdk", Role::Contract),
    ("aulo-config", Role::Adapter),
    ("aulo-store", Role::Adapter),
    ("aulo-telemetry", Role::Adapter),
    ("aulo-models", Role::Adapter),
    ("aulo-audio", Role::Adapter),
    ("aulo-speech-sherpa", Role::Adapter),
    ("aulo-speech-whisper", Role::Adapter),
    ("aulo-speech-system", Role::Adapter),
    ("aulo-speech-cloud", Role::Adapter),
    ("aulo-speech-bench", Role::Adapter),
    ("aulo-realtime", Role::Adapter),
    ("aulo-voice", Role::Domain),
    ("aulo-providers", Role::Assembly),
    ("aulo-agent", Role::Domain),
    ("aulo-policy", Role::Domain),
    ("aulo-tools", Role::Adapter),
    ("aulo-apps", Role::Adapter),
    ("aulo-browser", Role::Adapter),
    ("aulo-desktop-ax", Role::Adapter),
    ("aulo-plugin", Role::Adapter),
    ("aulo-server", Role::Assembly),
    ("aulo", Role::Surface),
    ("aulo-app", Role::Domain),
    ("aulo-ffi", Role::Surface),
    ("aulo-testkit", Role::Testkit),
];

/// Dependencies a contract crate must never name: contracts stay pure so every
/// other crate can depend on them without pulling in a runtime, a socket or a
/// platform library.
const CONTRACT_BANNED: &[&str] = &[
    "tokio",
    "async-std",
    "reqwest",
    "hyper",
    "ureq",
    "cpal",
    "rusqlite",
    "diesel",
    "diesel_migrations",
    "libsqlite3-sys",
    "wasmtime",
    "extism",
    "uniffi",
    "sherpa-onnx",
    "whisper-rs",
    "chromiumoxide",
];

/// Only surfaces own error presentation and argument parsing; libraries return
/// typed errors and take plain values.
const SURFACE_ONLY: &[&str] = &["clap", "anyhow"];

/// One owner crate per heavy or risky dependency family. An owner that is not
/// in the workspace yet is fine; any other crate naming the dependency is not.
/// Widen an owner list here, with the reason, when a second crate truly needs
/// the dependency.
const OWNERS: &[(&[&str], &[&str])] = &[
    (
        &["diesel", "diesel_migrations", "libsqlite3-sys"],
        &["aulo-store"],
    ),
    (&["prost", "prost-build", "tonic-prost"], &["aulo-proto"]),
    // The transport is needed wherever a server or client is built: aulo-server
    // serves, aulo-app is the desktop client. Messages still come from aulo-proto.
    (
        &["tonic", "tonic-build"],
        &["aulo-proto", "aulo-server", "aulo-app"],
    ),
    (&["tonic-health", "tonic-reflection"], &["aulo-server"]),
    (
        &["sherpa-onnx", "sherpa-onnx-sys", "sherpa-rs"],
        &["aulo-speech-sherpa"],
    ),
    (&["whisper-rs", "whisper-rs-sys"], &["aulo-speech-whisper"]),
    (&["wasmtime", "extism"], &["aulo-plugin"]),
    (&["chromiumoxide"], &["aulo-browser"]),
    (&["uniffi", "uniffi_bindgen", "uniffi_build"], &["aulo-ffi"]),
];

fn role_of(name: &str) -> Option<Role> {
    ROLES.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
}

/// Dependencies point down only: contract < domain/adapter < assembly <
/// surface. A domain crate never reaches an adapter, a surface is never a
/// dependency, and only a testkit may depend on a testkit.
fn may_depend_on(from: Role, to: Role) -> bool {
    use Role::{Adapter, Assembly, Contract, Domain, Surface, Testkit};
    match (from, to) {
        (_, Surface) => false,
        (Testkit, _) => true,
        (_, Testkit) => false,
        (Contract, to) => to == Contract,
        (Domain, to) => matches!(to, Contract | Domain),
        (Adapter, to) => matches!(to, Contract | Domain | Adapter),
        (Assembly | Surface, _) => true,
    }
}

fn unclassified_crates(graph: &Graph) -> Vec<String> {
    graph
        .keys()
        .filter(|name| role_of(name).is_none())
        .map(|name| format!("{name} has no role in the spec §4.2 table; add it to ROLES"))
        .collect()
}

fn layering_violations(graph: &Graph) -> Vec<String> {
    let mut found = Vec::new();
    for (name, deps) in graph {
        let Some(from) = role_of(name) else { continue };
        for dep in deps.iter().filter(|d| graph.contains_key(*d)) {
            let Some(to) = role_of(dep) else { continue };
            if !may_depend_on(from, to) {
                found.push(format!(
                    "{name} ({from:?}) must not depend on {dep} ({to:?})"
                ));
            }
        }
    }
    found
}

fn contract_io_violations(graph: &Graph) -> Vec<String> {
    let mut found = Vec::new();
    for (name, deps) in graph {
        if role_of(name) != Some(Role::Contract) {
            continue;
        }
        for dep in deps
            .iter()
            .filter(|d| CONTRACT_BANNED.contains(&d.as_str()))
        {
            found.push(format!(
                "{name} is a contract crate and must not depend on {dep}"
            ));
        }
    }
    found
}

fn surface_only_violations(graph: &Graph) -> Vec<String> {
    let mut found = Vec::new();
    for (name, deps) in graph {
        if role_of(name) == Some(Role::Surface) {
            continue;
        }
        for dep in deps.iter().filter(|d| SURFACE_ONLY.contains(&d.as_str())) {
            found.push(format!("{name} must not depend on {dep}; only surfaces do"));
        }
    }
    found
}

fn owner_violations(graph: &Graph) -> Vec<String> {
    let mut found = Vec::new();
    for (name, deps) in graph {
        for (family, owners) in OWNERS {
            if owners.contains(&name.as_str()) {
                continue;
            }
            for dep in deps.iter().filter(|d| family.contains(&d.as_str())) {
                found.push(format!(
                    "{name} must not depend on {dep}; only {} may",
                    owners.join(", ")
                ));
            }
        }
    }
    found
}

fn workspace_graph() -> Result<Graph, String> {
    // `--no-deps` lists workspace members only and needs no network.
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map_err(|e| format!("cargo metadata did not start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let meta: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata output is not JSON: {e}"))?;
    let packages = meta["packages"]
        .as_array()
        .ok_or("cargo metadata has no `packages` array")?;
    let mut graph = Graph::new();
    for package in packages {
        let name = package["name"].as_str().ok_or("package without a name")?;
        let deps = package["dependencies"]
            .as_array()
            .ok_or("package without a `dependencies` array")?
            .iter()
            // A null `kind` is a normal dependency; "dev" and "build" are not.
            .filter(|d| d["kind"].is_null())
            .filter_map(|d| d["name"].as_str().map(str::to_owned))
            .collect();
        graph.insert(name.to_owned(), deps);
    }
    Ok(graph)
}

fn assert_clean(violations: &[String]) {
    assert!(violations.is_empty(), "\n{}\n", violations.join("\n"));
}

#[test]
fn every_workspace_crate_has_a_role() {
    assert_clean(&unclassified_crates(&workspace_graph().unwrap()));
}

#[test]
fn dependencies_point_down_only() {
    assert_clean(&layering_violations(&workspace_graph().unwrap()));
}

#[test]
fn contracts_have_no_io_dependencies() {
    assert_clean(&contract_io_violations(&workspace_graph().unwrap()));
}

#[test]
fn only_surfaces_depend_on_clap_and_anyhow() {
    assert_clean(&surface_only_violations(&workspace_graph().unwrap()));
}

#[test]
fn heavy_dependencies_have_one_owner() {
    assert_clean(&owner_violations(&workspace_graph().unwrap()));
}

/// The rules above only matter if they fail on a violation, so each one is run
/// against a synthetic graph that breaks it.
#[test]
fn rules_catch_violations() {
    fn graph(rows: &[(&str, &[&str])]) -> Graph {
        rows.iter()
            .map(|(name, deps)| {
                (
                    (*name).to_owned(),
                    deps.iter().map(|d| (*d).to_owned()).collect(),
                )
            })
            .collect()
    }

    let g = graph(&[("aulo-types", &["reqwest"]), ("aulo-speech", &["tokio"])]);
    assert_eq!(
        contract_io_violations(&g),
        [
            "aulo-speech is a contract crate and must not depend on tokio",
            "aulo-types is a contract crate and must not depend on reqwest",
        ]
    );

    let g = graph(&[("aulo-store", &["clap"]), ("aulo", &["clap", "anyhow"])]);
    assert_eq!(
        surface_only_violations(&g),
        ["aulo-store must not depend on clap; only surfaces do"]
    );

    let g = graph(&[("aulo-store", &["diesel"]), ("aulo-agent", &["diesel"])]);
    assert_eq!(
        owner_violations(&g),
        ["aulo-agent must not depend on diesel; only aulo-store may"]
    );

    let g = graph(&[
        ("aulo-types", &["aulo-agent"]),
        ("aulo-agent", &["aulo-store"]),
        ("aulo-store", &["aulo"]),
        ("aulo-server", &["aulo-testkit"]),
        ("aulo-testkit", &[]),
        ("aulo", &[]),
    ]);
    assert_eq!(
        layering_violations(&g),
        [
            "aulo-agent (Domain) must not depend on aulo-store (Adapter)",
            "aulo-server (Assembly) must not depend on aulo-testkit (Testkit)",
            "aulo-store (Adapter) must not depend on aulo (Surface)",
            "aulo-types (Contract) must not depend on aulo-agent (Domain)",
        ]
    );

    assert_eq!(
        unclassified_crates(&graph(&[("aulo-mystery", &[])])),
        ["aulo-mystery has no role in the spec §4.2 table; add it to ROLES"]
    );
}
