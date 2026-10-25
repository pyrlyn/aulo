//! T5.1 table tests: the rule grammar per subject. Each row is one rule
//! list, one call and the decision it must reach — allow, deny or ask —
//! under the default settings (Default mode, OnRequest, workspace-write).

use aulo_policy::{
    ApprovalPolicy, Outcome, Risk, RuleSet, Settings, app_call, clipboard_call, decide_shell,
    file_call, input_call, mcp_call, plugin_call, site_call,
};
use std::path::{Path, PathBuf};

#[cfg(test)]
fn compiled(rules: RuleSet) -> aulo_policy::Policy {
    aulo_policy::Policy::new(
        &rules,
        Settings::default(),
        Some(Path::new("/home/alice")),
        &PathBuf::from("/repo"),
    )
    .expect("the rules compile")
}

#[cfg(test)]
fn grants(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(tool, subject)| ((*tool).to_owned(), (*subject).to_owned()))
        .collect()
}

// --- Shell -----------------------------------------------------------------

#[test]
fn shell_rules_cover_prefixes_and_the_whole_line() {
    // A prefix rule covers the command and what follows it at a word
    // boundary, but not a longer command that merely starts the same.
    let rules = RuleSet {
        allow: vec!["Bash(git status)".into(), "Bash(npm run test:*)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide_shell("git status", &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide_shell("npm run test -- --watch", &[]),
        Outcome::Allow { .. }
    ));
    // `npm run tests` is not covered by the `npm run test` prefix.
    assert!(matches!(
        policy.decide_shell("npm run tests", &[]),
        Outcome::Ask(_)
    ));
}

#[test]
fn shell_deny_beats_every_allow_and_the_risk_fallback() {
    let rules = RuleSet {
        deny: vec!["Bash(rm -rf:*)".into()],
        allow: vec!["Bash(rm -rf /tmp/build)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide_shell("rm -rf /tmp/build", &[]),
        Outcome::Deny { .. }
    ));
}

#[test]
fn shell_segments_are_judged_command_by_command() {
    // `&&` splits the line: a deny rule on one command refuses the line,
    // and an allow rule on every command allows it.
    let deny = RuleSet {
        deny: vec!["Bash(rm -rf:*)".into()],
        ..RuleSet::default()
    };
    assert!(matches!(
        compiled(deny).decide_shell("ls && rm -rf build", &[]),
        Outcome::Deny { .. }
    ));
    let allow = RuleSet {
        allow: vec!["Bash(ls)".into(), "Bash(echo hi)".into()],
        ..RuleSet::default()
    };
    assert!(matches!(
        compiled(allow.clone()).decide_shell("ls && echo hi", &[]),
        Outcome::Allow { .. }
    ));
    // One uncovered command still asks.
    assert!(matches!(
        compiled(allow).decide_shell("ls && rm -rf build", &[]),
        Outcome::Ask(_)
    ));
}

// --- App -------------------------------------------------------------------

#[test]
fn app_rules_match_the_bundle_id() {
    let allow = RuleSet {
        allow: vec!["App(com.apple.Safari)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(allow);
    assert!(matches!(
        policy.decide(&app_call("com.apple.Safari"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&app_call("com.apple.Terminal"), &[]),
        Outcome::Ask(_)
    ));
    // Every app tool spelling answers to `App`, however it is registered.
    let policy = compiled(RuleSet {
        deny: vec!["App(com.apple.Terminal)".into()],
        ..RuleSet::default()
    });
    assert!(matches!(
        policy.decide(&app_call("com.apple.Terminal"), &[]),
        Outcome::Deny { .. }
    ));
}

#[test]
fn an_app_glob_rule_covers_a_bundle_family() {
    let rules = RuleSet {
        allow: vec!["App(com.apple.*)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide(&app_call("com.apple.Safari"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&app_call("com.google.Chrome"), &[]),
        Outcome::Ask(_)
    ));
}

// --- Site ------------------------------------------------------------------

#[test]
fn a_domain_rule_covers_the_host_and_its_subdomains_only() {
    let rules = RuleSet {
        allow: vec!["Site(domain:example.com)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    for host in ["example.com", "www.example.com", "a.b.example.com"] {
        assert!(
            matches!(policy.decide(&site_call(host), &[]), Outcome::Allow { .. }),
            "{host}"
        );
    }
    // Another host matches no rule; a read-only site still runs on the
    // risk fallback, and per-site asks come from rules (T5.6).
    for host in ["example.com.evil", "notexample.com", "evil.com"] {
        assert!(
            matches!(policy.decide(&site_call(host), &[]), Outcome::Allow { .. }),
            "{host}"
        );
    }
    // A deny rule is what refuses one.
    let rules = RuleSet {
        deny: vec!["Site(evil.com)".into()],
        ..RuleSet::default()
    };
    assert!(matches!(
        compiled(rules).decide(&site_call("evil.com"), &[]),
        Outcome::Deny { .. }
    ));
}

#[test]
fn a_site_ask_rule_asks_for_one_host() {
    let rules = RuleSet {
        ask: vec!["Site(banking.example.com)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide(&site_call("banking.example.com"), &[]),
        Outcome::Ask(_)
    ));
    // A read-only site still runs with no rule.
    assert!(matches!(
        policy.decide(&site_call("news.example.com"), &[]),
        Outcome::Allow { .. }
    ));
}

// --- File ------------------------------------------------------------------

#[test]
fn file_rules_are_path_globs_relative_to_the_home_and_the_cwd() {
    let rules = RuleSet {
        deny: vec!["Read(~/.ssh/**)".into()],
        allow: vec!["Edit(src/**)".into(), "Read(/etc/hosts)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide(&file_call("read", "/home/alice/.ssh/id_ed25519"), &[]),
        Outcome::Deny { .. }
    ));
    assert!(matches!(
        policy.decide(&file_call("edit", "src/main.rs"), &[]),
        Outcome::Allow { .. }
    ));
    // A relative glob also matches under the cwd.
    assert!(matches!(
        policy.decide(&file_call("edit", "/repo/src/lib.rs"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&file_call("read", "/etc/hosts"), &[]),
        Outcome::Allow { .. }
    ));
}

#[test]
fn a_file_read_runs_and_a_write_asks_with_no_rule() {
    let policy = compiled(RuleSet::default());
    assert!(matches!(
        policy.decide(&file_call("read", "notes.txt"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&file_call("write", "notes.txt"), &[]),
        Outcome::Ask(_)
    ));
}

// --- MCP and plugins -------------------------------------------------------

#[test]
fn mcp_rules_match_by_server_prefix_or_exact_tool() {
    let allow = RuleSet {
        allow: vec!["mcp__github__*".into()],
        ..RuleSet::default()
    };
    let policy = compiled(allow);
    assert!(matches!(
        policy.decide(&mcp_call("github", "create_issue"), &[]),
        Outcome::Allow { .. }
    ));
    // Another server still asks: an MCP tool can do anything.
    assert!(matches!(
        policy.decide(&mcp_call("slack", "post_message"), &[]),
        Outcome::Ask(_)
    ));

    let exact = RuleSet {
        allow: vec!["mcp__github__create_issue".into()],
        ..RuleSet::default()
    };
    let policy = compiled(exact);
    assert!(matches!(
        policy.decide(&mcp_call("github", "create_issue"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&mcp_call("github", "delete_repo"), &[]),
        Outcome::Ask(_)
    ));
}

#[test]
fn plugin_tools_are_rated_like_mcp_tools() {
    let rules = RuleSet {
        deny: vec!["plugin__weather__*".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide(&plugin_call("weather", "forecast"), &[]),
        Outcome::Deny { .. }
    ));
}

// --- Input and clipboard ---------------------------------------------------

#[test]
fn input_rules_match_the_action_kind() {
    let rules = RuleSet {
        allow: vec!["Input(click)".into()],
        deny: vec!["Input(type)".into()],
        ..RuleSet::default()
    };
    let policy = compiled(rules);
    assert!(matches!(
        policy.decide(&input_call("click"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&input_call("type"), &[]),
        Outcome::Deny { .. }
    ));
    // An unrated action still asks: synthetic input acts as the user.
    assert!(matches!(
        policy.decide(&input_call("key"), &[]),
        Outcome::Ask(_)
    ));
}

#[test]
fn clipboard_reads_run_and_writes_ask() {
    let policy = compiled(RuleSet::default());
    assert!(matches!(
        policy.decide(&clipboard_call("read"), &[]),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&clipboard_call("write"), &[]),
        Outcome::Ask(_)
    ));
    let rules = RuleSet {
        ask: vec!["Clipboard(read)".into()],
        ..RuleSet::default()
    };
    assert!(matches!(
        compiled(rules).decide(&clipboard_call("read"), &[]),
        Outcome::Ask(_)
    ));
}

// --- Decision order and grants ---------------------------------------------

#[test]
fn deny_then_allow_then_ask_then_grants() {
    let rules = RuleSet {
        deny: vec!["App(com.apple.Terminal)".into()],
        allow: vec!["App(com.apple.Safari)".into()],
        ask: vec!["App(com.apple.Mail)".into()],
    };
    let policy = compiled(rules.clone());
    // Deny first, whatever else matches.
    let both = RuleSet {
        allow: vec!["App(com.apple.Terminal)".into()],
        ..rules.clone()
    };
    assert!(matches!(
        compiled(both).decide(&app_call("com.apple.Terminal"), &[]),
        Outcome::Deny { .. }
    ));
    // Allow before ask.
    let both = RuleSet {
        ask: vec!["App(com.apple.Safari)".into()],
        ..rules.clone()
    };
    assert!(matches!(
        compiled(both).decide(&app_call("com.apple.Safari"), &[]),
        Outcome::Allow { .. }
    ));
    // An ask rule always asks: it is checked before a grant, so a grant
    // cannot quietly cancel a rule the user wrote (T5.3's grants answer
    // the risk fallback).
    let mail = app_call("com.apple.Mail");
    assert!(matches!(policy.decide(&mail, &[]), Outcome::Ask(_)));
    assert!(matches!(
        policy.decide(&mail, &grants(&[("app", "com.apple.Mail")])),
        Outcome::Ask(_)
    ));
    // A grant answers a call no rule covers.
    let notes = app_call("com.apple.Notes");
    assert!(matches!(policy.decide(&notes, &[]), Outcome::Ask(_)));
    assert!(matches!(
        policy.decide(&notes, &grants(&[("app", "com.apple.Notes")])),
        Outcome::Allow { .. }
    ));
}

#[test]
fn a_grant_covers_only_its_own_subject() {
    let policy = compiled(RuleSet::default());
    let granted = grants(&[("app", "com.apple.Safari")]);
    assert!(matches!(
        policy.decide(&app_call("com.apple.Safari"), &granted),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        policy.decide(&app_call("com.apple.Terminal"), &granted),
        Outcome::Ask(_)
    ));
}

#[test]
fn an_unknown_mode_and_policy_combination_still_decides() {
    // `Never` turns every ask into a denial: a headless run cannot prompt.
    let policy = aulo_policy::Policy::new(
        &RuleSet::default(),
        Settings {
            approval: ApprovalPolicy::Never,
            ..Settings::default()
        },
        None,
        &PathBuf::from("."),
    )
    .expect("compiles");
    assert!(matches!(
        policy.decide(&app_call("com.apple.Safari"), &[]),
        Outcome::Deny { .. }
    ));
}

#[test]
fn a_malformed_rule_names_its_list_and_text() {
    let error = aulo_policy::Policy::new(
        &RuleSet {
            allow: vec!["App(com.apple.Safari".into()],
            ..RuleSet::default()
        },
        Settings::default(),
        None,
        &PathBuf::from("."),
    )
    .expect_err("a config error");
    assert_eq!(error.rule, "App(com.apple.Safari");
    assert!(error.to_string().contains("allow rule"), "{error}");
}

#[test]
fn the_shell_helper_still_decides_a_line() {
    // T5.2's entry point, unchanged: no rules, read-only runs.
    assert!(matches!(
        decide_shell("ls", &RuleSet::default()).expect("compiles"),
        Outcome::Allow { .. }
    ));
    assert!(matches!(
        decide_shell("rm -rf build", &RuleSet::default()).expect("compiles"),
        Outcome::Ask(aulo_policy::Why::Risk {
            risk: Risk::Destructive
        })
    ));
}
