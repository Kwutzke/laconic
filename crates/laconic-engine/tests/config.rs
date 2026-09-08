//! `laconic.toml` parsing, validation and layering.
//!
//! The done-when names three aborts explicitly — a malformed file, an unknown rule id, and an
//! override naming a language no pack claims — because each is a config the tool could otherwise
//! half understand, and half understanding one silently disables rules its author believed were on.

use laconic_engine::config::{ConfigError, ConfigFile, Thresholds, discover};
use laconic_engine::registry::Tier;
use std::path::Path;

const LANGUAGES: &[&str] = &["go", "python", "rust", "java", "typescript"];

fn parse(text: &str) -> ConfigFile {
    toml::from_str(text).expect("parses")
}

/// The four thresholds round-trip: stating their defaults resolves to the defaults.
///
/// **Not the whole of AC9.** `RuleConfig` carries one `tier` per rule while dispositions are per
/// kind, so a config stating `narration`'s default Line tier would also set its Doc tier to Gate
/// where the default is Warn — "a config stating only the defaults" is inexpressible for the rules
/// whose kinds differ. AC9 holds for the file laconic actually writes, because `defaults_toml`
/// emits `enabled` and `autofix` and never `tier`, and the CLI suite asserts that end to end by
/// comparing the output of two runs.
#[test]
fn stating_the_defaults_changes_nothing() {
    let empty = ConfigFile::default();
    let spelled = parse(
        r#"
        [thresholds]
        absolute_doc_lines = 6
        doc_lines_per_member = 3
        density_min_comment_lines = 8
        density_max_ratio = 0.5
        "#,
    );
    let from_empty = empty.resolve("go").thresholds;
    let from_spelled = spelled.resolve("go").thresholds;
    assert_eq!(from_empty, from_spelled);
    assert_eq!(from_empty, Thresholds::default());
}

/// A key the parser does not know is malformed, not ignored.
///
/// `deny_unknown_fields` is what makes a typo an error: `excluded_path` silently doing nothing is
/// the exact shape of failure the abort exists to prevent.
#[test]
fn an_unknown_key_is_malformed() {
    let err = toml::from_str::<ConfigFile>("excluded_path = [\"probe\"]\n");
    assert!(err.is_err(), "a mistyped key parsed as valid config");

    let bad_tier = toml::from_str::<ConfigFile>("[rules.banner]\ntier = \"blocking\"\n");
    assert!(bad_tier.is_err(), "an unknown tier name parsed as valid");
}

/// Unparseable TOML names the file it came from.
#[test]
fn a_malformed_file_reports_its_path() {
    let dir = tempdir();
    let path = dir.join("laconic.toml");
    std::fs::write(&path, "[rules\n").expect("write");
    match ConfigFile::load(&path) {
        Err(ConfigError::Malformed { path: p, .. }) => assert_eq!(p, path),
        other => panic!("expected Malformed, got {other:?}"),
    }
}

/// An unknown rule id aborts, and says which table it was in.
#[test]
fn an_unknown_rule_id_aborts() {
    let config = parse("[rules.nosuchrule]\nenabled = false\n");
    let errors = config.validate(LANGUAGES).expect_err("should not validate");
    assert_eq!(
        errors,
        vec![ConfigError::UnknownRule {
            id: "nosuchrule".to_string(),
            scope: "[rules]".to_string(),
        }]
    );
}

/// An override naming a language no pack claims aborts the same way.
#[test]
fn an_override_for_an_unclaimed_language_aborts() {
    let config = parse("[languages.cobol.rules.banner]\nenabled = false\n");
    let errors = config.validate(LANGUAGES).expect_err("should not validate");
    assert_eq!(
        errors,
        vec![ConfigError::UnknownLanguage {
            name: "cobol".to_string()
        }]
    );
}

/// Every problem in one pass, so a config with three typos takes one round trip rather than three.
#[test]
fn validation_reports_every_problem_at_once() {
    let config = parse(
        r#"
        [rules.nosuchrule]
        enabled = false
        [languages.cobol.rules.alsomissing]
        enabled = false
        "#,
    );
    let errors = config.validate(LANGUAGES).expect_err("should not validate");
    assert_eq!(errors.len(), 3, "got {errors:?}");
}

/// A language block layers over the global one, and reaches only its own language.
#[test]
fn a_language_override_reaches_one_language() {
    let config = parse(
        r#"
        [thresholds]
        absolute_doc_lines = 10

        [languages.python.thresholds]
        absolute_doc_lines = 20

        [languages.python.rules.banner]
        tier = "warn"
        "#,
    );
    config.validate(LANGUAGES).expect("valid");

    let go_resolved = config.resolve("go");
    let (go_rules, go) = (&go_resolved.registry, go_resolved.thresholds);
    let py_resolved = config.resolve("python");
    let (py_rules, py) = (&py_resolved.registry, py_resolved.thresholds);

    assert_eq!(go.absolute_doc_lines, 10, "global layer reaches go");
    assert_eq!(py.absolute_doc_lines, 20, "python's own layer wins");
    assert_eq!(
        go.doc_lines_per_member,
        Thresholds::default().doc_lines_per_member,
        "an unset threshold keeps its default rather than resetting"
    );

    let tier_of = |r: &laconic_engine::Registry| r.get("banner").unwrap().line.unwrap().tier;
    assert_eq!(tier_of(go_rules), Tier::Gate);
    assert_eq!(tier_of(py_rules), Tier::Warn);
}

/// Re-tiering leaves the fix shape alone, and does not switch a rule on for a kind it never ran on.
#[test]
fn re_tiering_does_not_change_applicability_or_fix_shape() {
    let config = parse("[rules.detached]\ntier = \"warn\"\n");
    let registry = config.resolve("go").registry;
    let entry = registry.get("detached").expect("detached exists");
    assert_eq!(entry.line.unwrap().tier, Tier::Warn);
    assert_eq!(
        entry.line.unwrap().fix,
        laconic_engine::FixShape::Delete,
        "tier moved, fix shape did not"
    );
    assert!(
        !entry.autofix,
        "re-tiering does not switch autofix on: `detached` ships Delete with autofix off, and \
         that pairing is the friction, not an oversight"
    );
    assert!(
        entry.doc.is_none(),
        "a kind the rule never applied to stays inapplicable"
    );
}

/// The excluded set replaces rather than extends, which is what lets a caller **remove** a shipped
/// default. The fixture suite is the case that needs it: every fixture lives under `testdata/`, so
/// it runs with the list cleared. Adding `probe/` is extension and would not have required this.
#[test]
fn the_excluded_set_is_replaced_not_extended() {
    let config = parse("excluded_paths = [\"probe\", \"vendor\"]\n");
    assert_eq!(
        config.excluded_paths(),
        Some(vec!["probe".to_string(), "vendor".to_string()])
    );
    assert_eq!(
        ConfigFile::default().excluded_paths(),
        None,
        "an absent list is not an empty one: the caller keeps its defaults"
    );
}

/// Discovery walks upward and stops at the first file.
#[test]
fn discovery_takes_the_nearest_file() {
    let dir = tempdir();
    let nested = dir.join("a").join("b");
    std::fs::create_dir_all(&nested).expect("mkdir");
    std::fs::write(dir.join("laconic.toml"), "").expect("write");
    assert_eq!(discover(&nested), Some(dir.join("laconic.toml")));

    std::fs::write(nested.join("laconic.toml"), "").expect("write");
    assert_eq!(
        discover(&nested),
        Some(nested.join("laconic.toml")),
        "the nearer file wins, rather than the two merging"
    );
}

/// A relative start walks up too.
///
/// `parent()` is lexical: `Path::new(".").parent()` is `Some("")` and `""`'s is `None`, so the walk
/// tested the starting directory twice and stopped inside it — reporting no config while one sat in
/// the parent, which is the silent half-understood-config outcome this module opens by refusing.
///
/// Serialised against the other discovery tests by using its own directory: the process-wide
/// current directory is what makes a relative path mean anything, and it cannot be set per test.
#[test]
fn discovery_walks_up_from_a_relative_path() {
    let dir = tempdir().join("relative");
    let nested = dir.join("a").join("b");
    std::fs::create_dir_all(&nested).expect("mkdir");
    std::fs::write(dir.join("laconic.toml"), "").expect("write");

    let cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&nested).expect("chdir");
    let found = discover(Path::new("."));
    std::env::set_current_dir(cwd).expect("chdir back");

    assert!(
        found.is_some_and(|p| p.ends_with("laconic.toml")),
        "a relative start found no config in an ancestor"
    );
}

#[test]
fn discovery_finds_nothing_when_there_is_nothing() {
    let dir = tempdir();
    assert_eq!(discover(&dir), None);
}

fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!(
        "laconic-config-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("mkdir");
    base
}
