//! The engine's per-file pipeline, driven by the Go pack.

use laconic_engine::domain::{Attachment, CommentKind, Visibility};
use laconic_engine::{Config, FileAnalysis, Skipped, analyse, resolve};
use laconic_packs::all;
use std::path::Path;

const SRC: &str = include_str!("../testdata/go/pipeline.go");

fn run() -> FileAnalysis {
    let packs = all();
    let path = Path::new("testdata/go/pipeline.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    // Path exclusions cleared: the fixture lives under `testdata/`, which the default config
    // excludes, so the suite would otherwise scan nothing at all.
    analyse(pack, grammar, path, SRC, &Config::unrestricted()).expect("fixture is analysable")
}

fn bodies(a: &FileAnalysis) -> Vec<String> {
    a.blocks.iter().map(|b| b.body()).collect()
}

fn block_starting(a: &FileAnalysis, prefix: &str) -> usize {
    a.blocks
        .iter()
        .position(|b| b.body().starts_with(prefix))
        .unwrap_or_else(|| panic!("no block starting {prefix:?}; got {:?}", bodies(a)))
}

/// A machine directive between prose and its declaration must not cost the block its Doc kind.
///
/// Go forces this shape — `//go:embed` must sit on the line directly above the declaration. As
/// Line kind the block carries a Delete fix at gate tier with autofix on, so `laconic fix` deletes
/// the godoc of an exported var. `no_delete_fix_applies_to_doc_kind` cannot catch it: that asserts
/// the registry table, not which row of the table a block is read from.
#[test]
fn a_directive_between_prose_and_its_declaration_keeps_doc_kind() {
    let src =
        "package x\n\n// Schema is the embedded DDL.\n//go:embed schema.sql\nvar Schema string\n";
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let block = a
        .blocks
        .iter()
        .find(|b| b.body().contains("embedded DDL"))
        .expect("the prose comment is extracted");
    assert_eq!(block.kind, CommentKind::Doc);
}

/// An interface method is a `method_elem`, not a `field_declaration`. Missing from the documentable
/// set, the comment above an exported interface method is Line kind — a Delete fix at gate tier
/// with autofix **on**, so `laconic fix` deletes the godoc of a published API.
#[test]
fn an_interface_method_comment_is_a_doc_comment() {
    let src = "package x\n\ntype Reader interface {\n\t// Read fills p.\n\tRead(p []byte) (int, error)\n}\n";
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let block = a
        .blocks
        .iter()
        .find(|b| b.body().contains("Read fills p"))
        .expect("the comment is extracted");
    assert_eq!(block.kind, CommentKind::Doc);
}

/// Only `var_declaration` wraps its specs in a `var_spec_list`; `const` and `type` list theirs as
/// direct children. The nested arm that unwraps the list was covered by no fixture — deleting it
/// left `var (\n\tFoo = 1\n)` declaring nothing, and `implInInterface` resolving nothing against a
/// grouped var block.
#[test]
fn a_grouped_var_block_declares_its_names() {
    let src = "package x\n\nvar (\n\tFoo = 1\n\tbar = 2\n)\n";
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let names: Vec<&str> = a.declared.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"Foo"), "got {names:?}");
    assert!(names.contains(&"bar"), "got {names:?}");
}

/// Concern 1 — resolution is per extension. A file no pack claims is skipped silently, because
/// laconic runs over whole repositories and warning on every `.json` makes the output unusable.
#[test]
fn pack_resolution_is_by_extension() {
    let packs = all();
    assert!(resolve(&packs, Path::new("x/y.go")).is_some());
    assert!(resolve(&packs, Path::new("x/y.json")).is_none());
    assert!(resolve(&packs, Path::new("Makefile")).is_none());
}

/// The default config excludes the directory AC2 puts fixtures in, which is why the exclusion set
/// has to be replaceable rather than only extendable.
#[test]
fn default_config_excludes_the_fixture_directory() {
    let packs = all();
    let path = Path::new("testdata/go/pipeline.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    assert_eq!(
        analyse(pack, grammar, path, SRC, &Config::default()).unwrap_err(),
        Skipped::ExcludedPath
    );
}

#[test]
fn generated_files_are_excluded_whole() {
    let packs = all();
    let path = Path::new("gen.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src =
        "// Code generated by protoc. DO NOT EDIT.\n\npackage gen\n\n// narration\nfunc F() {}\n";
    assert_eq!(
        analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap_err(),
        Skipped::GeneratedFile
    );
}

/// A machine directive is removed before any rule sees the block. This is also what protects Go
/// build constraints: no rule reaches one, so no rule needs a special case for it.
#[test]
fn machine_directives_never_reach_a_block() {
    let a = run();
    assert!(
        !bodies(&a).iter().any(|b| b.contains("go:build")),
        "got {:?}",
        bodies(&a)
    );
}

/// Top of file only. A mid-file notice still reaches `attribution`, which is the rule that has to
/// decide it.
#[test]
fn the_licence_header_is_carved_out() {
    let a = run();
    assert!(
        !bodies(&a).iter().any(|b| b.contains("Copyright")),
        "got {:?}",
        bodies(&a)
    );
}

/// Consecutive comments separated by whitespace only form one block; a blank line ends it. One
/// finding per block, one fix per block — a four-line narration is not four findings an autofixer
/// applies in four passes.
#[test]
fn adjacent_comments_form_one_block() {
    let a = run();
    let run_block = &a.blocks[block_starting(&a, " a first narration line")];
    assert_eq!(run_block.comments.len(), 2);
    assert_eq!(
        run_block.body(),
        " a first narration line\n a second narration line"
    );
    assert_eq!(run_block.line_count(), 2);

    // A blank line ends a run: these two are separate blocks despite both being `//` comments.
    assert_ne!(
        block_starting(&a, " a detached comment"),
        block_starting(&a, " this comment is protected")
    );
}

/// A comment with code before it on its line is always its own block and never merges — not with
/// the code beside it, and not with the comment below it.
#[test]
fn a_trailing_comment_is_its_own_block() {
    let a = run();
    let i = block_starting(&a, " a trailing comment");
    assert_eq!(a.blocks[i].comments.len(), 1);
    assert_eq!(a.blocks[i].attachment, Attachment::AttachedTrailing);
}

#[test]
fn attachment_has_three_states() {
    let a = run();
    assert_eq!(
        a.blocks[block_starting(&a, " Exported documents")].attachment,
        Attachment::AttachedBelow
    );
    assert_eq!(
        a.blocks[block_starting(&a, " a trailing comment")].attachment,
        Attachment::AttachedTrailing
    );
    assert_eq!(
        a.blocks[block_starting(&a, " a detached comment")].attachment,
        Attachment::Detached
    );
    assert_eq!(
        a.blocks[block_starting(&a, " a detached block comment")].attachment,
        Attachment::Detached
    );
}

/// Concern 5 and concern 6. Doc is positional in Go: the same node kind two lines above a
/// declaration is not a doc comment, which is why this cannot be a lexical test.
#[test]
fn kind_is_line_block_or_doc() {
    let a = run();
    assert_eq!(
        a.blocks[block_starting(&a, " Exported documents")].kind,
        CommentKind::Doc
    );
    assert_eq!(
        a.blocks[block_starting(&a, " a detached block comment")].kind,
        CommentKind::Block
    );
    assert_eq!(
        a.blocks[block_starting(&a, " a detached comment")].kind,
        CommentKind::Line
    );
    assert_eq!(
        a.blocks[block_starting(&a, " a trailing comment")].kind,
        CommentKind::Line
    );
}

#[test]
fn a_doc_block_carries_its_subject() {
    let a = run();
    let doc = &a.blocks[block_starting(&a, " Exported documents")];
    let subject = &a.subjects[doc.subject.expect("doc block has a subject")];
    assert_eq!(subject.visibility, Visibility::Exported);
    assert_eq!(
        subject.statement_count, 3,
        "Go nests statements one level deeper than `block`"
    );
    assert_eq!(
        subject.bound_identifiers,
        vec!["a", "error", "exported", "int"],
        "the header binds these; the body's identifiers are not the subject's"
    );

    // Not `Private`: an unexported top-level Go identifier is reachable from every other file in
    // the same package, which is exactly what `Restricted` describes and `Private` denies.
    let unexported = &a.blocks[block_starting(&a, " unexported does nothing")];
    let subject = &a.subjects[unexported.subject.unwrap()];
    assert_eq!(
        subject.visibility,
        Visibility::Restricted("package".to_string())
    );
}

/// A detached block is attached to nothing, so it has no subject — which is what makes `detached`
/// the only rule in the set with no text test at all, and the most destructive to run unattended.
#[test]
fn a_detached_block_has_no_subject() {
    let a = run();
    assert!(
        a.blocks[block_starting(&a, " a detached comment")]
            .subject
            .is_none()
    );
}

/// The directive is lifted out of the block it protects rather than merged into it. Otherwise its
/// text joins the block and changes what `narration` and `restate` match against, so a suppression
/// would alter the finding it suppresses.
#[test]
fn an_ignore_directive_is_lifted_out_and_binds_below() {
    let a = run();
    assert!(
        !bodies(&a).iter().any(|b| b.contains("laconic:ignore")),
        "the directive is not part of any block body: {:?}",
        bodies(&a)
    );

    let protected = &a.blocks[block_starting(&a, " this comment is protected")];
    let directive = protected
        .ignore
        .as_ref()
        .expect("bound to the block below it");
    assert_eq!(directive.rule, "narration");
    assert_eq!(
        directive.reason.as_deref(),
        Some("the line below is load-bearing")
    );
}

/// A directive with no reason parses, and its missing reason is data rather than a parse failure:
/// `ignoreReason` reports it at gate tier, so a run surfaces every one instead of aborting on the
/// first.
#[test]
fn a_directive_without_a_reason_is_data_not_an_error() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\n// laconic:ignore narration\n// changed to use a map\nfunc F() {}\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    let protected = &a.blocks[block_starting(&a, " changed to use a map")];
    let directive = protected.ignore.as_ref().expect("directive bound");
    assert_eq!(directive.rule, "narration");
    assert_eq!(directive.reason, None);
}

/// Concern 11 — every declaration in the file, so `implInInterface` can resolve a name in a doc
/// comment against something. Functions alone would resolve nothing for a private type, which is
/// the commonest name a public doc comment leaks.
#[test]
fn declared_symbols_span_more_than_functions() {
    let a = run();
    let got: Vec<(&str, &str, bool)> = a
        .declared
        .iter()
        .map(|d| (d.name.as_str(), d.form, d.visibility.is_exported()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Exported", "func", true),
            ("unexported", "func", false),
            ("multiline", "func", false),
            ("MaxDepth", "const", true),
            ("internalState", "type", false),
        ]
    );
}

/// A file that does not parse cleanly is processed, not skipped — a syntactically broken file that
/// read as clean would be silent in a pre-commit hook on partial staging.
#[test]
fn a_file_with_error_nodes_is_processed_and_flagged() {
    let packs = all();
    let path = Path::new("broken.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\n// narration about the next line\nfunc F( {}\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    assert!(a.has_error_nodes);
    assert!(
        !a.blocks.is_empty(),
        "comment extraction survives an ERROR node"
    );
}

/// A machine directive sitting **inside** a comment run must not split the run.
///
/// Removing directives before grouping breaks row adjacency, so the comments above and below one
/// stop being a single block — and a narration block split by a `//nolint:` line becomes two
/// gate-tier findings where the source has one comment.
#[test]
fn a_machine_directive_inside_a_run_does_not_split_the_block() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\nfunc f() {\n\t// first line\n\t//nolint:gosec\n\t// second line\n\tq := 1\n\t_ = q\n}\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    let blocks: Vec<&laconic_engine::CommentBlock> = a
        .blocks
        .iter()
        .filter(|b| b.body().contains("line"))
        .collect();
    assert_eq!(blocks.len(), 1, "one block, not two: {:?}", bodies(&a));
    assert_eq!(blocks[0].comments.len(), 2, "the directive is gone from it");
    assert!(!blocks[0].body().contains("nolint"));
}

/// A multi-byte character straddling the generated-marker cut must not panic. laconic runs as a
/// pre-commit hook and in CI, where a panic is the only output anyone sees.
#[test]
fn a_multibyte_character_near_the_marker_cut_does_not_panic() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let mut src = String::from("package x\n\nvar s = \"");
    while src.len() < 2040 {
        src.push('a');
    }
    // Pushed so that the character spans the 2048-byte cut.
    src.push_str("日本語日本語");
    src.push_str("\"\n");
    let a = analyse(pack, grammar, path, &src, &Config::unrestricted());
    assert!(a.is_ok());
}

/// The package comment is a doc comment. As Line kind it would carry a Delete fix at gate tier with
/// autofix on, so `laconic fix` would delete the surface pkg.go.dev renders.
#[test]
fn the_package_comment_is_doc_kind() {
    let a = run();
    let pkg = &a.blocks[block_starting(&a, " Package pipeline")];
    assert_eq!(pkg.kind, CommentKind::Doc);
}

/// A trailing comment documents nothing. Without the alone-on-its-line test it becomes the doc
/// comment of whatever declaration follows it.
#[test]
fn a_trailing_comment_is_never_a_doc_comment() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\nvar a = 1 // trailing\nfunc F() {}\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    let trailing = &a.blocks[block_starting(&a, " trailing")];
    assert_eq!(trailing.kind, CommentKind::Line);
    assert_eq!(trailing.attachment, Attachment::AttachedTrailing);
}

/// A spec binds N names and the grouped form nests one level deeper. Reading the first `name` field
/// of the first spec drops everything else, and `implInInterface` cannot resolve what it dropped.
#[test]
fn declared_symbols_cover_grouped_and_multi_name_declarations() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\nvar Foo, bar = 1, 2\n\nconst (\n\tAlpha = 1\n\tbeta  = 2\n)\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    let got: Vec<(&str, &str)> = a
        .declared
        .iter()
        .map(|d| (d.form, d.name.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("var", "Foo"),
            ("var", "bar"),
            ("const", "Alpha"),
            ("const", "beta"),
        ]
    );
}

/// A directive that protects nothing still lacks a reason, and is the one a reader most needs told
/// about. Dropping unbound directives made `ignoreReason` silently incomplete.
#[test]
fn a_directive_binding_to_no_block_is_retained() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\nfunc f() {\n\t// laconic:ignore narration\n\n\tq := 1\n\t_ = q\n}\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    assert_eq!(a.unbound_directives.len(), 1);
    assert_eq!(a.unbound_directives[0].rule, "narration");
    assert_eq!(a.unbound_directives[0].reason, None);
}

/// The licence carve-out must not swallow a package comment that merely mentions a licence. The
/// marker has to open a line, and a doc comment is never carved out.
#[test]
fn the_licence_carve_out_spares_a_package_comment() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "// Package x implements the MIT-licensed parser.\npackage x\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    assert!(
        bodies(&a).iter().any(|b| b.contains("Package x")),
        "the package comment survives: {:?}",
        bodies(&a)
    );
}

/// `line_count` counts rows, not comments. A regression to counting comments halves the number
/// `density`'s 8-line threshold and `docbloat`'s 15-line threshold are measured against.
#[test]
fn line_count_counts_rows_of_a_multi_row_block_comment() {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let src = "package x\n\nfunc f() {\n\t/* one\n\t   two\n\t   three */\n\tq := 1\n\t_ = q\n}\n";
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    let block = a
        .blocks
        .iter()
        .find(|b| b.body().contains("one"))
        .expect("the block comment");
    assert_eq!(block.comments.len(), 1);
    assert_eq!(block.line_count(), 3);
}
