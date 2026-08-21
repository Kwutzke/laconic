//! §7's partition: what each rule does when the file does not parse cleanly.
//!
//! A file with ERROR nodes is **processed, not skipped**. Skipping it would make a syntactically
//! broken file read as clean, which in a pre-commit hook on partial staging is silent.

use laconic_engine::{
    Config, Registry, Report, Rules, all_block_rules, all_subject_rules, analyse, dispatch, resolve,
};
use laconic_packs::all;
use std::path::Path;

fn report_for(src: &str) -> Report {
    let packs = all();
    let path = Path::new("broken.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let analysis =
        analyse(pack, grammar, path, src, &Config::unrestricted()).expect("processed, not skipped");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let (findings, note) = dispatch(path, src, &analysis, &Registry::default(), &rules);
    let mut report = Report {
        findings,
        withheld: note.into_iter().collect(),
        unreadable: Vec::new(),
    };
    report.finalise();
    report
}

fn withheld(report: &Report) -> Vec<&'static str> {
    report
        .withheld
        .first()
        .map(|n| n.rules.clone())
        .unwrap_or_default()
}

fn fired(report: &Report, rule: &str) -> bool {
    report.active().any(|f| f.rule == rule)
}

/// A subtree that does not parse, carrying one block of each kind the four subtree-scoped rules
/// need: a doc comment for `docbloat`, and line comments inside the body for the rest. Attachment
/// and subject resolution are both unreliable here.
const BROKEN_SUBTREE: &str = "\
package broken

// Bad does one thing.
// It also has a second line.
func Bad( {
\t// changed to use a map

\t// the login, never the display form
\tx := 1
}
";

/// One function fails to parse; a second, exported one is clean and its doc comment names a private
/// type declared in the same file.
const BROKEN_ELSEWHERE: &str = "\
package broken

type internalCache struct{}

// Get reads through internalCache when the entry is cold.
func Get(k string) string {
\treturn k
}

func bad( {}
";

#[test]
fn a_broken_file_is_processed_rather_than_skipped() {
    let report = report_for(BROKEN_SUBTREE);
    assert!(
        !report.withheld.is_empty() || !report.findings.is_empty(),
        "comment extraction survives an ERROR node"
    );
}

/// The four subtree-scoped rules. Each is named in the withheld note, which is what makes a file
/// laconic could not fully analyse distinguishable from a clean one.
#[test]
fn subtree_scoped_rules_are_withheld_and_named() {
    let report = report_for(BROKEN_SUBTREE);
    let names = withheld(&report);
    for rule in ["restate", "detached", "docbloat", "density"] {
        assert!(
            names.contains(&rule),
            "{rule} is withheld and named; note said {names:?}"
        );
        assert!(!fired(&report, rule), "{rule} did not fire");
    }
}

/// `implInInterface` alone is withheld for the **whole file**, not just the broken subtree. Its
/// input is file-scoped — concern 11 enumerates every declaration — so a clean subtree says nothing
/// about whether that enumeration is complete. Under subtree suppression this rule would run
/// against a partial declaration set, fail to resolve the name, and report nothing, with no note to
/// distinguish that from a clean file.
#[test]
fn impl_in_interface_is_withheld_for_the_whole_file() {
    let clean = "\
package broken

type internalCache struct{}

// Get reads through internalCache when the entry is cold.
func Get(k string) string {
\treturn k
}
";
    assert!(
        fired(&report_for(clean), "implInInterface"),
        "the rule fires when the file parses"
    );

    let report = report_for(BROKEN_ELSEWHERE);
    assert!(
        !fired(&report, "implInInterface"),
        "and is withheld when anything in the file does not"
    );
    assert!(withheld(&report).contains(&"implInInterface"));
}

/// The nine unaffected rules keep working: their test is over comment text, or over the comment
/// body's own nested parse, neither of which depends on the surrounding tree.
#[test]
fn text_rules_are_unaffected_by_a_broken_parse() {
    let report = report_for(BROKEN_SUBTREE);
    assert!(fired(&report, "narration"), "narration reads only the body");
    assert!(!withheld(&report).contains(&"narration"));
}

/// `commentedOutCode` is in the same subtask as the subject-dependent rules and in the opposite half
/// of the partition: it parses the comment body itself, so the surrounding tree is irrelevant to it.
#[test]
fn commented_out_code_is_unaffected_by_a_broken_parse() {
    let src = "\
package broken

func bad( {
\t// y := a + 1
\tx := 1
}
";
    let report = report_for(src);
    assert!(fired(&report, "commentedOutCode"));
    assert!(!withheld(&report).contains(&"commentedOutCode"));
}

/// A parse failure never by itself produces a non-zero exit. The compiler owns syntax errors;
/// laconic reports what it can still see.
#[test]
fn a_parse_failure_alone_does_not_gate() {
    let src = "package broken\n\nfunc bad( {}\n";
    let report = report_for(src);
    assert!(
        report
            .active()
            .all(|f| f.tier != laconic_engine::Tier::Gate)
    );
    assert_eq!(report.exit_code(), laconic_engine::EXIT_CLEAN);
}
