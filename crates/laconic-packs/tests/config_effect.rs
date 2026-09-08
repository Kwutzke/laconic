//! A threshold override reaches the rule that reads it, per language.
//!
//! `crates/laconic-engine/tests/config.rs` asserts what the file resolves to. This asserts the half
//! that matters to a caller: that resolving it changes which findings a run produces, and that a
//! `[languages.python]` block does not quietly retune Go.

use laconic_engine::{
    Config, ConfigFile, Rules, all_block_rules, all_subject_rules, analyse, dispatch, resolve,
};
use laconic_packs::all;
use std::path::Path;

const LANGUAGES: &[&str] = &["go", "python", "rust", "java", "typescript"];

/// Seven doc-comment lines on a one-member struct: over the shipped cap of six, and over the
/// per-member ratio too, so both of `docbloat`'s arms fire under the defaults.
const SRC: &str = "\
package p

// Widget is a widget.
// It has a name.
// The name is a string.
// The string is UTF-8.
// UTF-8 is a unicode encoding.
// Unicode is a standard.
// Standards are useful.
type Widget struct {
\tName string
}
";

fn docbloat_fires(config: &ConfigFile) -> bool {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let analysis = analyse(pack, grammar, path, SRC, &Config::unrestricted()).expect("analysable");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let resolved = config.resolve(pack.name());
    let (findings, _) = dispatch(path, SRC, &analysis, &resolved, &rules);
    findings.iter().any(|f| f.rule == "docbloat")
}

fn parse(text: &str) -> ConfigFile {
    toml::from_str(text).expect("parses")
}

/// The baseline every other case here is measured against.
#[test]
fn the_shipped_defaults_fire_on_this_comment() {
    assert!(docbloat_fires(&ConfigFile::default()));
}

/// Raising both arms silences it, which is only possible if the threshold is read at check time.
#[test]
fn a_raised_threshold_silences_the_rule() {
    let config = parse("[thresholds]\nabsolute_doc_lines = 40\ndoc_lines_per_member = 40\n");
    assert!(!docbloat_fires(&config));
}

/// The mechanism this issue exists for: retuned for one language, untouched in the others.
#[test]
fn a_language_override_reaches_only_its_own_pack() {
    let go =
        parse("[languages.go.thresholds]\nabsolute_doc_lines = 40\ndoc_lines_per_member = 40\n");
    go.validate(LANGUAGES).expect("valid");
    assert!(!docbloat_fires(&go), "go's own block did not reach go");

    let python = parse(
        "[languages.python.thresholds]\nabsolute_doc_lines = 40\ndoc_lines_per_member = 40\n",
    );
    python.validate(LANGUAGES).expect("valid");
    assert!(docbloat_fires(&python), "a python override retuned go");
}

/// Disabling a rule in config stops it dispatching at all.
#[test]
fn a_disabled_rule_produces_no_finding() {
    let config = parse("[rules.docbloat]\nenabled = false\n");
    assert!(!docbloat_fires(&config));
}
