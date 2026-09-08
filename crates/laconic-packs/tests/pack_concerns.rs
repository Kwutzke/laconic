//! The four packs beyond Go, against the concerns each one answers differently.
//!
//! One test per asymmetry rather than per pack, because the asymmetries are the content: a pack
//! that agreed with Go on everything would need no test at all.

use laconic_engine::domain::{Attachment, CommentKind, Visibility};
use laconic_engine::{
    Config, FileAnalysis, Registry, Rules, all_block_rules, all_subject_rules, analyse, dispatch,
    resolve,
};
use laconic_packs::all;
use std::path::Path;

fn analyse_str(name: &str, src: &str) -> FileAnalysis {
    let packs = all();
    let path = Path::new(name);
    let (pack, grammar) = resolve(&packs, path).unwrap_or_else(|| panic!("no pack claims {name}"));
    analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable")
}

/// Every rule id the full set reports for one source, so a test can name the rule it expects and
/// the ones it expects nothing from.
fn rules_fired(name: &str, src: &str) -> Vec<&'static str> {
    let packs = all();
    let path = Path::new(name);
    let (pack, grammar) = resolve(&packs, path).unwrap_or_else(|| panic!("no pack claims {name}"));
    let analysis = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let (findings, _) = dispatch(path, src, &analysis, &Registry::default(), &rules);
    findings.into_iter().map(|f| f.rule).collect()
}

fn block_with<'a>(a: &'a FileAnalysis, needle: &str) -> &'a laconic_engine::CommentBlock {
    a.blocks
        .iter()
        .find(|b| b.body().contains(needle))
        .unwrap_or_else(|| {
            panic!(
                "no block containing {needle:?}; got {:?}",
                a.blocks.iter().map(|b| b.body()).collect::<Vec<_>>()
            )
        })
}

/// Concern 1 — every extension the five packs claim resolves, and to the right grammar. TS/JS is
/// three grammars behind one pack, which is the whole reason resolution is per extension.
#[test]
fn every_claimed_extension_resolves() {
    let packs = all();
    let expected = [
        ("a.go", "go"),
        ("a.py", "python"),
        ("a.pyi", "python"),
        ("a.rs", "rust"),
        ("a.java", "java"),
        ("a.ts", "typescript"),
        ("a.tsx", "typescript"),
        ("a.js", "typescript"),
        ("a.mjs", "typescript"),
        ("a.cjs", "typescript"),
        ("a.jsx", "typescript"),
        ("a.mts", "typescript"),
        ("a.cts", "typescript"),
    ];
    for (file, pack_name) in expected {
        let (pack, _) = resolve(&packs, Path::new(file)).unwrap_or_else(|| panic!("{file}"));
        assert_eq!(pack.name(), pack_name, "{file}");
    }

    let ts = resolve(&packs, Path::new("a.ts")).unwrap().1;
    let tsx = resolve(&packs, Path::new("a.tsx")).unwrap().1;
    let js = resolve(&packs, Path::new("a.js")).unwrap().1;
    assert_ne!(ts, tsx);
    assert_ne!(tsx, js);
    assert_ne!(ts, js);
}

/// Python's doc comment is not a comment node, and it documents the scope that **contains** it.
#[test]
fn a_python_docstring_documents_its_enclosing_scope() {
    let src = "def exported(a):\n    \"\"\"Function docstring.\"\"\"\n    return a\n";
    let a = analyse_str("x.py", src);
    let doc = block_with(&a, "Function docstring");
    assert_eq!(doc.kind, CommentKind::Doc);
    let subject = &a.subjects[doc.subject.expect("a docstring has a subject")];
    assert_eq!(subject.visibility, Visibility::Exported);
    // The subject is the function, so the docstring sits inside its span rather than above it.
    assert!(subject.span.start <= doc.span.start && doc.span.end <= subject.span.end);
}

/// A tuple variant and a newtype each declare one member, so four lines of prose above one is the
/// shape `docbloat` catches. Reverses `MIN_BODY_ROWS`, which hid the class while rows were the
/// denominator.
#[test]
fn one_member_is_a_denominator() {
    let src = "pub enum Shape {\n    /// A circle.\n    ///\n    /// The radius unit is metres,\n    /// which callers get wrong.\n    Circle(f64),\n}\n\n/// A newtype.\n///\n/// The invariant is not visible\n/// from the type alone.\npub struct Metres(f64);\n";
    assert_eq!(
        rules_fired("x.rs", src)
            .iter()
            .filter(|r| **r == "docbloat")
            .count(),
        2,
        "the variant and the newtype both fire; got {:?}",
        rules_fired("x.rs", src)
    );
}

/// Java and TS/JS make a machine directive the same node kind as prose, so the pack's own
/// doc-comment lookup must walk over one rather than answering with it. Answered with it, the
/// marker test fails, the declaration reads as undocumented, and its javadoc or JSDoc drops to a
/// kind carrying a Delete fix at gate tier with autofix on.
#[test]
fn a_directive_does_not_hide_the_doc_comment_above_it() {
    let java = "public class F {\n    /** Reads the row. */\n    // CHECKSTYLE:OFF\n    public int read() { return 0; }\n}\n";
    let a = analyse_str("F.java", java);
    assert_eq!(block_with(&a, "Reads the row").kind, CommentKind::Doc);

    let ts = "/** Looks up the key. */\n// eslint-disable-next-line no-explicit-any\nexport function lookup(k: string) { return k; }\n";
    let b = analyse_str("x.ts", ts);
    assert_eq!(block_with(&b, "Looks up the key").kind, CommentKind::Doc);
}

/// A Rust statement-level attribute is a named child of `block`, so counting it inflates
/// `density`'s denominator by one per attribute. The probe pins this as a grammar fact; nothing
/// pinned the pack's own exclusion.
///
/// **Both forms**, because the exclusion is a two-element list and an outer attribute alone leaves
/// `inner_attribute_item` free to be deleted with the suite green.
#[test]
fn a_rust_attribute_is_not_a_statement() {
    let with_attr = "fn f() {\n    #![allow(dead_code)]\n    #[allow(unused)]\n    let x = 1;\n    let _ = x;\n}\n";
    let without = "fn f() {\n    let x = 1;\n    let _ = x;\n}\n";
    let count = |src: &str| {
        analyse_str("x.rs", src)
            .subjects
            .iter()
            .map(|s| s.member_count)
            .max()
            .unwrap_or(0)
    };
    assert_eq!(count(with_attr), 2, "two statements, attribute excluded");
    assert_eq!(count(with_attr), count(without));
}

/// And the docstring is discounted from the statement count, so the same shape counts the same in
/// Python as anywhere else.
#[test]
fn a_python_docstring_is_not_a_statement() {
    let with_doc = "def f():\n    \"\"\"Doc.\"\"\"\n    x = 1\n    return x\n";
    let without = "def f():\n    x = 1\n    return x\n";
    let a = analyse_str("x.py", with_doc);
    let b = analyse_str("x.py", without);
    let count = |a: &FileAnalysis| {
        a.subjects
            .iter()
            .filter(|s| s.member_count > 0)
            .map(|s| s.member_count)
            .max()
            .unwrap_or(0)
    };
    assert_eq!(
        count(&a),
        count(&b),
        "the docstring is not counted as a statement"
    );
}

/// Python has no visibility construct, so a leading underscore is the answer — and it is
/// `Restricted`, not `Private`: nothing stops an importer reaching it.
#[test]
fn python_visibility_is_a_convention() {
    let src = "def exported():\n    pass\n\n\ndef _internal():\n    pass\n";
    let a = analyse_str("x.py", src);
    let by_name = |n: &str| {
        a.declared
            .iter()
            .find(|d| d.name == n)
            .unwrap_or_else(|| panic!("{n} not declared"))
    };
    assert_eq!(by_name("exported").visibility, Visibility::Exported);
    assert_eq!(
        by_name("_internal").visibility,
        Visibility::Restricted("module".to_string())
    );
}

/// Rust reads the grammar's `doc` field. `////////` is a line comment, not a doc comment, and the
/// pack never inspects a marker to know that.
#[test]
fn rust_doc_comments_come_from_the_field_not_the_marker() {
    let src = "/// Documents it.\npub fn f() {}\n\n////////\n\npub fn g() {}\n";
    let a = analyse_str("x.rs", src);
    assert_eq!(block_with(&a, "Documents it").kind, CommentKind::Doc);
    let banner = block_with(&a, "//////");
    assert_eq!(
        banner.kind,
        CommentKind::Line,
        "a run of slashes is not documentation"
    );
}

/// `pub(self)` is a `visibility_modifier` on a strictly private item, so presence is not the fact —
/// the modifier's text is.
#[test]
fn rust_visibility_reads_the_modifier_text() {
    let src = "pub fn a() {}\npub(crate) fn b() {}\npub(self) fn c() {}\nfn d() {}\n";
    let a = analyse_str("x.rs", src);
    let vis = |n: &str| {
        a.declared
            .iter()
            .find(|d| d.name == n)
            .map(|d| d.visibility.clone())
            .unwrap_or_else(|| panic!("{n} not declared"))
    };
    assert_eq!(vis("a"), Visibility::Exported);
    assert_eq!(vis("b"), Visibility::Restricted("pub(crate)".to_string()));
    assert_eq!(vis("c"), Visibility::Private);
    assert_eq!(vis("d"), Visibility::Private);
}

/// A variant inherits its enum's visibility, because Rust forbids `pub` on one. Read as an absent
/// modifier it was Private, and `implInInterface` then fired on any public doc comment using the
/// words a public enum's variants are named after — sixteen findings on this repository's own
/// source, every one a false positive.
#[test]
fn rust_enum_variants_inherit_the_enums_visibility() {
    let src = "pub enum Kind {\n    Line,\n    Block,\n}\n\nenum Hidden {\n    Inner,\n}\n";
    let a = analyse_str("x.rs", src);
    let vis = |n: &str| {
        a.declared
            .iter()
            .find(|d| d.name == n)
            .map(|d| d.visibility.clone())
            .unwrap_or_else(|| panic!("{n} not declared"))
    };
    assert_eq!(vis("Line"), Visibility::Exported);
    assert_eq!(vis("Block"), Visibility::Exported);
    assert_eq!(
        vis("Inner"),
        Visibility::Private,
        "a variant of a private enum stays private"
    );
}

/// A Rust doc comment is attached to the item below it. The grammar's `doc` child carries the
/// trailing newline, so the comment node's own end row is one past its text — read directly, every
/// Rust doc comment resolves as Detached.
#[test]
fn a_rust_doc_comment_is_attached_to_its_item() {
    let src = "/// Documents it.\npub fn f() {}\n";
    let a = analyse_str("x.rs", src);
    assert_eq!(
        block_with(&a, "Documents it").attachment,
        Attachment::AttachedBelow
    );
}

/// Java's doc comment is marker text, and the marker test excludes the empty comment and the
/// all-asterisk banner that `banner` exists to catch.
#[test]
fn java_doc_comments_are_marker_text_but_not_every_marker() {
    let src = "/** Documents it. */\npublic class A {}\n\n/****/\nclass B {}\n";
    let a = analyse_str("x.java", src);
    assert_eq!(block_with(&a, "Documents it").kind, CommentKind::Doc);
    // Looked up by source span: `/****/` strips to `*`, so its body says nothing useful.
    let banner = a
        .blocks
        .iter()
        .find(|b| src[b.span.clone()].starts_with("/****/"))
        .expect("the all-asterisk block");
    assert_eq!(
        banner.kind,
        CommentKind::Block,
        "an all-asterisk comment is a banner, not documentation"
    );
}

/// No modifier is package-private in Java — reachable from other files, so `Restricted`.
#[test]
fn java_visibility_has_four_answers() {
    let src =
        "public class A {\n  public int a;\n  protected int b;\n  private int c;\n  int d;\n}\n";
    let a = analyse_str("x.java", src);
    let vis = |n: &str| {
        a.declared
            .iter()
            .find(|d| d.name == n)
            .map(|d| d.visibility.clone())
            .unwrap_or_else(|| panic!("{n} not declared; got {:?}", a.declared))
    };
    assert_eq!(vis("a"), Visibility::Exported);
    assert_eq!(vis("b"), Visibility::Restricted("protected".to_string()));
    assert_eq!(vis("c"), Visibility::Private);
    assert_eq!(vis("d"), Visibility::Restricted("package".to_string()));
}

/// A JSDoc comment sits above `export function f`, so its sibling is the `export_statement` and not
/// the declaration it documents.
#[test]
fn a_jsdoc_comment_reaches_through_the_export_statement() {
    // Per grammar, because `: void` is a TypeScript return-type annotation and the JavaScript
    // grammar has no node for one. Running one source through all three asserted the JS case
    // against a file that grammar rejects, passing on error-recovery output.
    for (name, src) in [
        (
            "x.ts",
            "/** Documents it. */\nexport function f(): void {}\n",
        ),
        (
            "x.tsx",
            "/** Documents it. */\nexport function f(): void {}\n",
        ),
        ("x.js", "/** Documents it. */\nexport function f() {}\n"),
    ] {
        let a = analyse_str(name, src);
        assert!(
            !a.has_error_nodes,
            "{name}: the source must parse under its own grammar"
        );
        let doc = block_with(&a, "Documents it");
        assert_eq!(doc.kind, CommentKind::Doc, "{name}");
        let subject = &a.subjects[doc.subject.unwrap_or_else(|| panic!("{name}: no subject"))];
        assert_eq!(subject.visibility, Visibility::Exported, "{name}");
    }
}

/// `html_comment` is a comment. Leaving it out of concern 2 would make `<!-- changed to use a
/// map -->` invisible to every rule.
#[test]
fn an_html_comment_is_extracted() {
    let src = "<!-- changed to use a map -->\nexport const x = 1;\n";
    for name in ["x.ts", "x.js"] {
        let a = analyse_str(name, src);
        let block = block_with(&a, "changed to use a map");
        assert_eq!(
            block.kind,
            CommentKind::Line,
            "{name}: it runs to end of line"
        );
    }
}

/// `const` and `let` bind through a `variable_declarator`, so a declaration walk alone never sees
/// them — and a module-private const is exactly the name a public JSDoc leaks.
#[test]
fn typescript_declared_symbols_include_bindings() {
    let src = "export const Shown = 1;\nconst hidden = 2;\n";
    let a = analyse_str("x.ts", src);
    let vis = |n: &str| {
        a.declared
            .iter()
            .find(|d| d.name == n)
            .map(|d| d.visibility.clone())
            .unwrap_or_else(|| panic!("{n} not declared; got {:?}", a.declared))
    };
    assert_eq!(vis("Shown"), Visibility::Exported);
    assert_eq!(vis("hidden"), Visibility::Restricted("module".to_string()));
}

/// Concern 9 for TypeScript, which had none while Go had three.
///
/// The allowlist decides whether `docbloat`'s ratio and `density` engage at all, so dropping
/// `interface_body` left a documented interface counting zero members with the suite green. The
/// conformance module catches an entry that cannot occur; this catches one that occurs uncounted.
#[test]
fn a_typescript_type_counts_what_it_declares() {
    let members_of = |name: &str, src: &str, head: &str| {
        let a = analyse_str(name, src);
        a.subjects
            .iter()
            .find(|s| src[s.span.clone()].starts_with(head))
            .unwrap_or_else(|| panic!("{name}: no subject starting {head:?}"))
            .member_count
    };

    let iface = "export interface Store {\n  get(k: string): void;\n  put(k: string): void;\n}\n";
    assert_eq!(members_of("x.ts", iface, "interface Store"), 2);

    let class = "export class Row {\n  id = \"\";\n  name = \"\";\n  touch(): void {}\n}\n";
    assert_eq!(members_of("x.ts", class, "class Row"), 3);

    let enom = "export enum Mode {\n  Retry,\n  Fail,\n}\n";
    assert_eq!(members_of("x.ts", enom, "enum Mode"), 2);

    // A type alias names no `body` field at all, so it reaches the count through neither branch.
    let alias = "export type ID = string;\n";
    assert_eq!(members_of("x.ts", alias, "type ID"), 0);
}
