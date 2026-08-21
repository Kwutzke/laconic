//! The four packs beyond Go, against the concerns each one answers differently.
//!
//! One test per asymmetry rather than per pack, because the asymmetries are the content: a pack
//! that agreed with Go on everything would need no test at all.

use laconic_engine::domain::{Attachment, CommentKind, Visibility};
use laconic_engine::{Config, FileAnalysis, analyse, resolve};
use laconic_packs::all;
use std::path::Path;

fn analyse_str(name: &str, src: &str) -> FileAnalysis {
    let packs = all();
    let path = Path::new(name);
    let (pack, grammar) = resolve(&packs, path).unwrap_or_else(|| panic!("no pack claims {name}"));
    analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable")
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
        ("a.jsx", "typescript"),
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
            .filter(|s| s.statement_count > 0)
            .map(|s| s.statement_count)
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
    let src = "/** Documents it. */\nexport function f(): void {}\n";
    for name in ["x.ts", "x.tsx", "x.js"] {
        let a = analyse_str(name, src);
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
