//! The fixture suite AC2 mandates, and the runner every later rule and pack extends.
//!
//! **Expectations live in a sidecar `.expected` file, not in an annotation comment.** An inline
//! `// want ...` annotation is itself a comment, so it would be extracted, grouped, dispatched and
//! very likely reported — a fixture format that changes the thing it measures. The sidecar costs a
//! second file per fixture and cannot do that.
//!
//! There is no bless mode. A runner that rewrites its own expectations turns a regression into a
//! diff nobody reads, and the green-seeking agent this whole design is written against finds that
//! affordance immediately. On a mismatch the runner prints the actual lines for a human to paste.

use laconic_engine::{
    Config, Report, Resolved, Rules, all_block_rules, all_subject_rules, analyse, dispatch, resolve,
};
use laconic_packs::all;
use std::fs;
use std::path::{Path, PathBuf};

fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

/// One `line:rule` per finding, sorted — the whole report, not just the rule the directory is named
/// for. A fixture that trips a second rule is showing something true, and hiding it would let two
/// rules fire on one comment unnoticed.
fn actual(path: &Path) -> Vec<String> {
    let src = fs::read_to_string(path).expect("fixture is readable");
    let packs = all();
    let (pack, grammar) = resolve(&packs, path).expect("a pack claims this fixture");
    // Path exclusions cleared: fixtures live under `testdata/`, which the default config excludes.
    let analysis = analyse(pack, grammar, path, &src, &Config::unrestricted())
        .expect("a fixture is never skipped");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let (findings, note) = dispatch(path, &src, &analysis, &Resolved::default(), &rules);
    let mut report = Report {
        findings,
        withheld: note.into_iter().collect(),
        unreadable: Vec::new(),
    };
    report.finalise();
    report
        .active()
        .map(|f| format!("{}:{}", f.line, f.rule))
        .collect()
}

fn instructions(path: &Path) -> Vec<String> {
    let src = fs::read_to_string(path).expect("fixture is readable");
    let packs = all();
    let (pack, grammar) = resolve(&packs, path).expect("a pack claims this fixture");
    let analysis = analyse(pack, grammar, path, &src, &Config::unrestricted()).expect("analysable");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let (findings, _) = dispatch(path, &src, &analysis, &Resolved::default(), &rules);
    findings.into_iter().map(|f| f.instruction).collect()
}

fn expected(path: &Path) -> Vec<String> {
    let sidecar = path.with_extension("expected");
    let text = fs::read_to_string(&sidecar)
        .unwrap_or_else(|e| panic!("{} is missing: {e}", sidecar.display()));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn fixtures_for(rule: &str) -> Vec<PathBuf> {
    let dir = testdata().join("go").join(rule);
    let mut out: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} is missing: {e}", dir.display()))
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "go"))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "{} has no fixtures", dir.display());
    out
}

/// AC2: every rule has positive **and** negative fixtures. A rule with only positives is a rule
/// nobody checked for false positives, which is the failure mode this whole design is defensive
/// about.
#[test]
fn every_rule_has_a_positive_and_a_negative_fixture() {
    for rule in RULES {
        let names: Vec<String> = fixtures_for(rule)
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(
            names.contains(&"positive".to_string()),
            "{rule} has no positive fixture"
        );
        assert!(
            names.contains(&"negative".to_string()),
            "{rule} has no negative fixture"
        );
    }
}

/// Every rule with a fixture directory. `ignoreReason` and `deadIgnore` are here too: they are
/// ordinary entries in the registry, not engine behaviour with no configuration surface.
const RULES: &[&str] = &[
    "narration",
    "banner",
    "attribution",
    "hedging",
    "vague",
    "task",
    "fileref",
    "restate",
    "detached",
    "commentedOutCode",
    "docbloat",
    "implInInterface",
    "density",
    "ignoreReason",
    "deadIgnore",
];

/// Every mismatch is reported, not just the first. A runner that stops at the first difference
/// makes a change touching several rules take one round trip per rule to understand.
#[test]
fn every_fixture_matches_its_expectations() {
    let mut failures = Vec::new();
    for rule in RULES {
        for path in fixtures_for(rule) {
            let (got, want) = (actual(&path), expected(&path));
            if got != want {
                failures.push(format!(
                    "{}\n  expected: {:?}\n  actual:   {:?}",
                    path.strip_prefix(testdata()).unwrap_or(&path).display(),
                    want,
                    got
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} fixture(s) disagree with their expectations:\n\n{}\n",
        failures.len(),
        failures.join("\n\n")
    );
}

/// AC8: every diagnostic states the change to make. A message that names a defect without naming
/// the change fails this criterion regardless of how accurate it is — the consumer is a machine
/// acting on one diagnostic, and it will act on whatever the sentence tells it to do.
#[test]
fn every_instruction_opens_with_the_change_to_make() {
    // Every verb any rule opens with. The criterion is that the sentence names a change; the list
    // grows when a rule needs a verb it does not have, and never to accommodate a message that
    // names a defect instead.
    const IMPERATIVES: &[&str] = &[
        "remove",
        "rewrite",
        "add",
        "reference",
        "replace",
        "delete",
        "move",
        "shorten",
        "reduce",
    ];
    let mut seen = 0;
    for rule in RULES {
        let path = testdata().join("go").join(rule).join("positive.go");
        for instruction in instructions(&path) {
            let first = instruction
                .split_whitespace()
                .next()
                .expect("an instruction is never empty")
                .to_lowercase();
            assert!(
                IMPERATIVES.contains(&first.as_str()),
                "{rule}: {instruction:?} opens with {first:?}, which names no change"
            );
            seen += 1;
        }
    }
    assert!(seen >= RULES.len(), "every rule contributed an instruction");
}

/// A negative fixture reports nothing at all. Stated separately from the sidecar comparison so the
/// suite fails loudly if a negative fixture ever gains an expectation by accident.
#[test]
fn negative_fixtures_report_nothing() {
    for rule in RULES {
        let path = testdata().join("go").join(rule).join("negative.go");
        assert!(
            expected(&path).is_empty(),
            "{}: a negative fixture expects no findings",
            path.display()
        );
        assert_eq!(actual(&path), Vec::<String>::new(), "{}", path.display());
    }
}
