//! T5.2 fixtures: the classifier's risk is the decision when no rule matches.

use aulo_policy::{
    DecidedBy, Outcome, Risk, RuleSet, SHELL_COMMAND_MAX_BYTES, Why, decide_shell, shell_call,
};

#[test]
fn rm_rf_is_destructive_and_asks() {
    let call = shell_call("rm -rf");
    assert_eq!(call.risk, Risk::Destructive);
    let outcome = decide_shell("rm -rf", &RuleSet::default()).expect("empty rules compile");
    assert!(matches!(
        outcome,
        Outcome::Ask(Why::Risk {
            risk: Risk::Destructive
        })
    ));
}

#[test]
fn ls_is_read_only() {
    let call = shell_call("ls");
    assert_eq!(call.risk, Risk::ReadOnly);
    let outcome = decide_shell("ls", &RuleSet::default()).expect("empty rules compile");
    assert!(matches!(
        outcome,
        Outcome::Allow {
            by: DecidedBy::Policy
        }
    ));
}

#[test]
fn a_deny_rule_refuses_read_only_ls() {
    let rules = RuleSet {
        deny: vec!["Shell(ls)".into()],
        ..RuleSet::default()
    };
    let outcome = decide_shell("ls", &rules).expect("rule compiles");
    assert!(matches!(
        outcome,
        Outcome::Deny {
            by: DecidedBy::Rule,
            ..
        }
    ));
}

#[test]
fn an_allow_rule_permits_a_destructive_line() {
    let rules = RuleSet {
        allow: vec!["Bash(rm -rf /tmp/build)".into()],
        ..RuleSet::default()
    };
    let outcome = decide_shell("rm -rf /tmp/build", &rules).expect("rule compiles");
    assert!(matches!(
        outcome,
        Outcome::Allow {
            by: DecidedBy::Rule
        }
    ));
}

#[test]
fn a_command_past_the_cap_asks_and_drops_the_text() {
    let command = format!("ls {}", "a".repeat(SHELL_COMMAND_MAX_BYTES));
    let call = shell_call(&command);
    assert_eq!(call.risk, Risk::Exec);
    assert!(call.subject.is_empty());
    let segments = call.segments.expect("over-cap call is segmented");
    assert!(segments.opaque);
    assert!(segments.commands.is_empty());
    let outcome = decide_shell(&command, &RuleSet::default()).expect("empty rules compile");
    assert!(matches!(
        outcome,
        Outcome::Ask(Why::Risk { risk: Risk::Exec })
    ));
}
