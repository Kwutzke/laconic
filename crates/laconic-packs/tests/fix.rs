//! AC5 and AC6 for `laconic fix`, over every fixture in the corpus.
//!
//! AC5 is conditional by design: a file carrying ERROR nodes is still processed, so the claim is
//! that fixing introduces no error it did not find. AC6 is idempotence, which catches a collapse
//! eating a line per pass.
//!
//! **AC6 does not catch an orphaned directive** — `deadIgnore` is Rewrite and `fix` applies only
//! Delete, so the text is identical either way. The assertions below catch it.

use laconic_engine::{
    Config, Registry, Resolved, Rules, all_block_rules, all_subject_rules, analyse, dispatch, fix,
    resolve,
};
use laconic_packs::all;
use std::path::{Path, PathBuf};

/// Every source file under `testdata/`, whatever language claims it.
fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e != "expected") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// One pass: the fixed text, and whether the input parsed clean.
fn fix_once(path: &Path, src: &str) -> Option<(String, bool)> {
    let packs = all();
    let (pack, grammar) = resolve(&packs, path)?;
    let analysis = analyse(pack, grammar, path, src, &Config::unrestricted()).ok()?;
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let registry = Registry::default();
    let (findings, _) = dispatch(
        path,
        src,
        &analysis,
        &Resolved::from(registry.clone()),
        &rules,
    );
    let clean = !analysis.has_error_nodes;
    Some((fix(src, &analysis, &findings, &registry, pack), clean))
}

/// AC5. A file that parsed clean still parses clean after fixing.
#[test]
fn fixing_introduces_no_parse_errors() {
    let mut checked = 0;
    for path in corpus() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some((fixed, clean_before)) = fix_once(&path, &src) else {
            continue;
        };
        if !clean_before || fixed == src {
            continue;
        }
        let (_, clean_after) =
            fix_once(&path, &fixed).expect("the fixed text resolves the same way");
        assert!(
            clean_after,
            "{}: fixing introduced a parse error",
            path.display()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no fixture in the corpus produced a fix, so AC5 asserted nothing"
    );
}

/// AC6. A second pass changes nothing.
#[test]
fn fixing_twice_is_fixing_once() {
    let mut changed = 0;
    for path in corpus() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some((once, _)) = fix_once(&path, &src) else {
            continue;
        };
        let (twice, _) = fix_once(&path, &once).expect("the fixed text resolves the same way");
        assert_eq!(
            once,
            twice,
            "{}: a second fix pass changed the file",
            path.display()
        );
        if once != src {
            changed += 1;
        }
    }
    assert!(
        changed > 0,
        "no fixture in the corpus produced a fix, so AC6 asserted nothing"
    );
}

/// One pass over an in-memory source, for the cases a fixture cannot hold.
fn fixed(name: &str, src: &str) -> String {
    let packs = all();
    let path = Path::new(name);
    let (pack, grammar) = resolve(&packs, path).unwrap_or_else(|| panic!("no pack claims {name}"));
    let analysis = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let registry = Registry::default();
    let (findings, _) = dispatch(
        path,
        src,
        &analysis,
        &Resolved::from(registry.clone()),
        &rules,
    );
    fix(src, &analysis, &findings, &registry, pack)
}

/// Doc comments above declaration kinds the packs once left off their lists. Each was Block kind,
/// and `narration` deleted it at gate tier.
#[test]
fn a_doc_comment_on_a_once_missing_declaration_survives_fix() {
    let cases = [
        (
            "x.java",
            "public enum Mode {\n    /** Updated to mean \"retry once\". */\n    RETRY,\n}\n",
        ),
        (
            "x.ts",
            "/** Updated to use a Map. */\nexport const lookup = (k: string) => cache.get(k);\n",
        ),
    ];
    for (name, src) in cases {
        assert_eq!(fixed(name, src), src, "{name}: fix removed a doc comment");
    }
}

/// Doc comments whose declaration sits under a parent the pack's table does not name, or under a
/// chain of `export` and `declare` wrappers. Each was Block kind, and `narration` deleted it.
#[test]
fn a_doc_comment_under_an_unlisted_or_wrapped_parent_survives_fix() {
    let cases = [
        // tree-sitter recovers this by putting the method under an ERROR node.
        (
            "x.ts",
            "export class A {\n  a(: {\n  /** Updated to handle nulls. */\n  m(): void {}\n}\n",
        ),
        (
            "x.java",
            "class A {\n    void f(int k) {\n        switch (k) {\n            case 1:\n                /** Updated to retry once. */\n                class L {}\n                break;\n        }\n    }\n}\n",
        ),
        (
            "x.ts",
            "export default {\n  /** Changed to load lazily. */\n  load() {},\n};\n",
        ),
        (
            "x.ts",
            "/** Updated to accept a Map. */\nexport declare function lookup(k: string): void;\n",
        ),
        (
            "x.ts",
            "/** Changed at build time. */\ndeclare const VERSION: string;\n",
        ),
        (
            "x.ts",
            "declare global {\n  /** Changed to a boolean. */\n  var __DEV__: boolean;\n}\nexport {};\n",
        ),
    ];
    for (name, src) in cases {
        assert_eq!(fixed(name, src), src, "{name}: fix removed a doc comment");
    }
}

/// A Swift doc comment is marker text above any member kind, and `narration` never deletes one.
#[test]
fn a_swift_doc_comment_survives_fix_on_every_member_kind() {
    let src = "\
public extension Array {
    /// Updated to skip the first element.
    func rest() -> ArraySlice<Element> { dropFirst() }
}

public protocol Source {
    /// Changed to be async.
    func poll() async
}

public enum Mode {
    /// Updated to mean \"retry once\".
    case retry
}
";
    assert_eq!(fixed("x.swift", src), src);
}

/// A narration comment in a Swift body is deleted, and what remains still parses.
#[test]
fn fixing_swift_leaves_a_file_that_parses() {
    let src = "\
public func load() -> Int {
    // changed to use a map
    let x = 1
    return x
}
";
    let once = fixed("x.swift", src);
    assert!(!once.contains("changed to use"), "{once}");
    let tree = laconic_grammars::Grammar::Swift
        .parser()
        .parse(&once, None)
        .expect("parses");
    assert!(!tree.root_node().has_error(), "{once}");
}

/// Code **after** a block comment on its line survives the fix.
///
/// The whole-line branch had one guard, for code *before* the comment, so
/// `/* ---- helpers ---- */ func f() {}` — `banner` at gate tier with autofix on — took the
/// declaration with it. Neither AC5 nor AC6 could see it: the residue parses and a second pass is a
/// no-op, so the deletion was silent in exactly the mode that writes files.
#[test]
fn a_block_comment_never_takes_the_code_beside_it() {
    let once = fixed("x.go", "package x\n\n/* ---- helpers ---- */ func f() {}\n");
    assert!(
        once.contains("func f() {}"),
        "fix deleted the declaration:\n{once}"
    );
    assert!(!once.contains("helpers"), "the banner survived:\n{once}");
    assert!(
        once.contains("\nfunc f() {}"),
        "the declaration kept a leading space:\n{once:?}"
    );
    assert_eq!(once, fixed("x.go", &once), "second pass changed the file");
}

/// The same guard from the other side, which was already right and must stay so.
#[test]
fn a_trailing_comment_still_keeps_the_code_before_it() {
    let src = "package x\n\nfunc f() {\n\tn := 1 // changed to use a map\n\t_ = n\n}\n";
    let once = fixed("x.go", src);
    assert!(once.contains("n := 1"), "{once}");
    assert!(!once.contains("changed to use a map"), "{once}");
    assert!(
        !once.contains("n := 1 \n"),
        "a trailing space was left behind: {once:?}"
    );
}

/// Code on **both** sides keeps the gap between them.
///
/// The two trims were independent, so both fired and the tokens joined: `if /* n */ x > 1` fixed to
/// `ifx > 1`. `var x /* n */ int = 1` is the worse shape — it becomes `var xint = 1`, which still
/// parses, so AC5 stayed green while a declaration was renamed.
#[test]
fn a_comment_between_two_tokens_leaves_them_separated() {
    let once = fixed(
        "x.go",
        "package x\n\nfunc f(x int) int {\n\tif /* changed to use a map */ x > 1 {\n\t\treturn 2\n\t}\n\treturn x\n}\n",
    );
    assert!(once.contains("\tif x > 1 {"), "tokens joined:\n{once}");
    assert!(!once.contains("changed to use a map"), "{once}");

    let decl = fixed(
        "x.go",
        "package x\n\nfunc f() {\n\tvar x /* changed to use a map */ int = 1\n\t_ = x\n}\n",
    );
    assert!(
        decl.contains("var x int = 1"),
        "declaration renamed:\n{decl}"
    );
}

/// A directive naming a *different* rule is the case that orphans one.
///
/// Suppression is per rule, so a block carrying `laconic:ignore restate` still reports `banner` and
/// still gets deleted. Leaving the directive behind makes the next run report `deadIgnore`, a
/// finding `fix` manufactured — caught by the assertion below rather than by AC6, which cannot see
/// it: `deadIgnore` is Rewrite, `fix` applies only Delete, so the text is identical either way.
#[test]
fn a_removed_block_takes_its_directive_with_it() {
    let src = "package x\n\nfunc f() int {\n\t// laconic:ignore restate — the names are the point\n\t// ---- helpers ----\n\treturn 1\n}\n";
    let once = fixed("x.go", src);
    assert!(!once.contains("helpers"), "the banner survived: {once:?}");
    assert!(
        !once.contains("laconic:ignore"),
        "the directive outlived the block it protected: {once:?}"
    );
    assert_eq!(once, fixed("x.go", &once), "second pass changed the file");
}

/// A directive naming the rule that fired suppresses it, so nothing is fixed.
#[test]
fn a_suppressed_finding_is_not_fixed() {
    let src = "package x\n\nfunc f() int {\n\t// laconic:ignore banner — the divider is load-bearing\n\t// ---- helpers ----\n\treturn 1\n}\n";
    assert_eq!(fixed("x.go", src), src);
}

/// A trailing comment loses the comment, not the line: the code before it is not the finding.
#[test]
fn a_trailing_comment_keeps_its_code() {
    let src = "package x\n\nfunc f() int {\n\tn := 1 // changed to use a map\n\treturn n\n}\n";
    let once = fixed("x.go", src);
    assert!(
        once.contains("\tn := 1\n"),
        "the code went with it: {once:?}"
    );
    assert!(
        !once.contains("changed to"),
        "the comment survived: {once:?}"
    );
    assert_eq!(once, fixed("x.go", &once));
}

/// Concern 10 is the pack's answer, and the two answers differ observably.
///
/// The same shape in both languages is what makes this a test of the policy rather than of one
/// language's formatting.
#[test]
fn blank_line_handling_is_the_packs_answer() {
    let go = "package x\n\nfunc f() int {\n\tn := 1\n\n\t// ---- helpers ----\n\n\treturn n\n}\n";
    assert_eq!(
        fixed("x.go", go),
        "package x\n\nfunc f() int {\n\tn := 1\n\n\treturn n\n}\n",
        "Go collapses the run"
    );

    let py = "def f():\n    n = 1\n\n    # changed to use a map\n\n    return n\n";
    assert_eq!(
        fixed("x.py", py),
        "def f():\n    n = 1\n\n\n    return n\n",
        "Python leaves the surrounding blanks"
    );
}

/// `CollapseRun` takes the whole run below, not one line of it.
///
/// Both sides tested a single line, so a longer run kept everything past the first blank. The
/// blanks *above* are deliberately untouched — taking one would let a removal under a declaration
/// pull that declaration up — which is why three above stay three.
#[test]
fn collapsing_takes_the_whole_run_below() {
    let go = "package x\n\nfunc f() int {\n\tn := 1\n\n\n\n\t// ---- helpers ----\n\n\n\n\treturn n\n}\n";
    assert_eq!(
        fixed("x.go", go),
        "package x\n\nfunc f() int {\n\tn := 1\n\n\n\n\treturn n\n}\n",
        "the run below should be gone and the run above untouched"
    );

    // One on each side is the case the policy was named for, and there it does reach one.
    let single =
        "package x\n\nfunc f() int {\n\tn := 1\n\n\t// ---- helpers ----\n\n\treturn n\n}\n";
    assert_eq!(
        fixed("x.go", single),
        "package x\n\nfunc f() int {\n\tn := 1\n\n\treturn n\n}\n"
    );
}
