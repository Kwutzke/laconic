//! The engine's per-file pipeline, driven by the Go pack.

use laconic_engine::domain::{Attachment, CommentKind, Visibility};
use laconic_engine::{
    ABSOLUTE_DOC_LINES, Config, DENSITY_MIN_COMMENT_LINES, DOC_LINES_PER_MEMBER, FileAnalysis,
    Finding, Resolved, Rules, Skipped, all_block_rules, all_subject_rules, analyse, dispatch,
    resolve,
};
use laconic_packs::all;
use std::path::Path;

/// Every rule id the full set reports for one Go source.
fn rules_fired(src: &str) -> Vec<&'static str> {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let analysis = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let (findings, _) = dispatch(path, src, &analysis, &Resolved::default(), &rules);
    findings.into_iter().map(|f| f.rule).collect()
}

/// The full findings for one Go source, where the message text is what is under test.
fn findings_for(src: &str) -> Vec<Finding> {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let analysis = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    dispatch(path, src, &analysis, &Resolved::default(), &rules).0
}

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

/// Both of `interface_type`'s element kinds, since neither is a `field_declaration`: a method is a
/// `method_elem` and an embedded interface is a `type_elem`. Missing from the documentable
/// set, the comment above an exported interface member is Line kind — a Delete fix at gate tier
/// with autofix **on**, so `laconic fix` deletes the godoc of a published API.
#[test]
fn an_interface_element_comment_is_a_doc_comment() {
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

    // An embedded interface element is the other half of `interface_type`, and was the half left
    // out when `method_elem` was added.
    let embedded = "package x\n\ntype ReadWriter interface {\n\t// Reader supplies the read half.\n\tReader\n}\n";
    let b = analyse(pack, grammar, path, embedded, &Config::unrestricted()).expect("analysable");
    let block = b
        .blocks
        .iter()
        .find(|b| b.body().contains("read half"))
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
        subject.member_count, 3,
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

fn members_of(src: &str, head: &str) -> usize {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).expect("go pack claims .go");
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).expect("analysable");
    a.subjects
        .iter()
        .find(|s| src[s.span.clone()].starts_with(head))
        .unwrap_or_else(|| {
            panic!(
                "no subject starting {head:?}; got {:?}",
                a.subjects
                    .iter()
                    .map(|s| src[s.span.clone()].lines().next().unwrap_or(""))
                    .collect::<Vec<_>>()
            )
        })
        .member_count
}

/// Concern 9 for the type forms, which reach their members through no `body` field at all.
///
/// An interface's elements are `method_elem` and `type_elem` — an embedded interface counts, since
/// it is part of the contract a caller reads. A struct reaches its fields through
/// `field_declaration_list`, and a `const` block through its specs.
#[test]
fn a_go_type_counts_what_it_declares() {
    let iface = "package x\n\ntype Store interface {\n\tio.Closer\n\tGet(k string) error\n\tPut(k string) error\n}\n";
    assert_eq!(members_of(iface, "type Store"), 3);

    let strukt = "package x\n\ntype Row struct {\n\tID string\n\tName string\n}\n";
    assert_eq!(members_of(strukt, "type Row"), 2);

    // A grouped block and a single declaration each count what they declare. Only `var` nests its
    // specs a level further down, in a `var_spec_list`; `const` lists them directly in both forms.
    let grouped = "package x\n\nconst (\n\tA = 1\n\tB = 2\n\tC = 3\n)\n";
    assert_eq!(members_of(grouped, "const ("), 3);
    let single = "package x\n\nconst A = 1\n";
    assert_eq!(members_of(single, "const A"), 1);

    // Every other type form declares none, so only `docbloat`'s absolute cap reaches it.
    let alias = "package x\n\ntype ID string\n";
    assert_eq!(members_of(alias, "type ID"), 0);

    // `var` is the shape that nests: its grouped form wraps the specs in a `var_spec_list`, which
    // is the only reason `spec_count` descends at all. Exercising `const` alone leaves that branch
    // untouched, and collapsing it to direct children then silently counts every grouped `var`
    // block as zero members.
    let grouped_var = "package x\n\nvar (\n\tA = 1\n\tB = 2\n\tC = 3\n)\n";
    assert_eq!(members_of(grouped_var, "var ("), 3);
    let single_var = "package x\n\nvar A = 1\n";
    assert_eq!(members_of(single_var, "var A"), 1);
    // A spec binds more than one name at a time, and each is a member.
    let multi = "package x\n\nvar (\n\tA, B = 1, 2\n)\n";
    assert_eq!(members_of(multi, "var ("), 1);
}

/// A `package_clause` declares no members, which is what keeps an ordinary package comment quiet
/// without `docbloat` carrying a special case for it.
#[test]
fn a_package_clause_has_no_members() {
    let src = "// Package x parses the wire format.\n//\n// It is deliberately allocation-free, so\n// every entry point takes a caller buffer.\npackage x\n";
    assert_eq!(members_of(src, "package x"), 0);
    assert!(
        !rules_fired(src).contains(&"docbloat"),
        "a four-line package comment is not bloat: {:?}",
        rules_fired(src)
    );
}

/// The finding this whole change exists to produce: five lines of prose above a one-method
/// interface. Under a row denominator the interface measured 3 — `interface {`, the signature, `}`
/// — which put the threshold at nine lines and made the shape unreportable.
#[test]
fn a_one_method_interface_is_a_denominator() {
    let src = "package x\n\n// CustomerLister exposes the customer-side enumeration the backfill\n// pipeline drives in its Prepare stage — the canonical list of customer\n// ids that exist. Satisfied by an ACL adapter wrapping\n// customerpublic.CustomerEnumerator. Per-customer purchase reads still\n// go through CustomerOrderArticleReader.\ntype CustomerLister interface {\n\tListIDs(ctx context.Context) ([]string, error)\n}\n";
    assert_eq!(members_of(src, "type CustomerLister"), 1);
    assert!(
        rules_fired(src).contains(&"docbloat"),
        "got {:?}",
        rules_fired(src)
    );
}

/// `density` was structurally blind to interfaces: the divide-by-zero guard exempted every one.
/// The commentary is detached rather than doc because `density` excludes Doc kind on purpose.
#[test]
fn density_reaches_an_interface() {
    let src = "package x\n\ntype Store interface {\n\t// The two halves below are ordered by how\n\t// often they are called rather than by name,\n\t// which is a convention this package keeps\n\t// and no other package in the tree does.\n\t// A reader coming from elsewhere will look\n\t// for alphabetical order and not find it.\n\t// The ordering is load-bearing for the\n\t// generated mock, which emits in source\n\t// order and is diffed in review.\n\n\tGet(k string) error\n\tPut(k string) error\n}\n";
    assert_eq!(members_of(src, "type Store"), 2);
    assert!(
        rules_fired(src).contains(&"density"),
        "nine comment lines against two methods; got {:?}",
        rules_fired(src)
    );
}

/// `line_count` counts rows, not comments. A regression to counting comments halves the number
/// every line threshold is measured against. The values live on the constants; naming them here
/// would be a second copy, and the last one went stale the first time the cap moved.
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

/// Every numeric threshold, pinned at its own boundary — silent at the value, firing one past it.
///
/// Built *from* each constant, because a fixture stating a number is the thing that goes stale. The
/// values themselves are pinned once, in the engine's own defaults test.
#[test]
fn each_threshold_fires_one_line_past_itself() {
    // A subject declaring no members meets the cap alone, which isolates it from the ratio.
    let alias_with = |lines: usize| {
        let prose = (0..lines)
            .map(|i| format!("// Line {i} of prose about the identifier below.\n"))
            .collect::<String>();
        format!("package x\n\n{prose}type ID string\n")
    };
    assert!(
        !rules_fired(&alias_with(ABSOLUTE_DOC_LINES)).contains(&"docbloat"),
        "the cap is the largest count that stays silent"
    );
    assert!(
        rules_fired(&alias_with(ABSOLUTE_DOC_LINES + 1)).contains(&"docbloat"),
        "one line past the cap must fire"
    );

    // One member is the only denominator at which the ratio decides anything the cap has not
    // already decided: at two members the ratio's threshold has reached the cap.
    let iface_with = |lines: usize| {
        let prose = (0..lines)
            .map(|i| format!("// Line {i} of prose about the interface below.\n"))
            .collect::<String>();
        format!("package x\n\n{prose}type S interface {{\n\tGet() error\n}}\n")
    };
    assert!(
        !rules_fired(&iface_with(DOC_LINES_PER_MEMBER)).contains(&"docbloat"),
        "at the ratio exactly, the rule is silent"
    );
    assert!(
        rules_fired(&iface_with(DOC_LINES_PER_MEMBER + 1)).contains(&"docbloat"),
        "one line past the ratio must fire, below the cap"
    );
    // The band where the ratio decides anything at all: above one member its threshold has already
    // reached the cap, so a change to it moves nothing. Asserted through the constants so a
    // calibration pass that closes the band is told, rather than finding the ratio quietly inert.
    assert!(
        (1..=1).any(|m: usize| m * DOC_LINES_PER_MEMBER < ABSOLUTE_DOC_LINES),
        "the ratio reaches below the cap at one member, or it decides nothing anywhere"
    );

    // `density` counts non-doc commentary, so the run sits inside the interface body.
    let commented_iface = |lines: usize| {
        let prose = (0..lines)
            .map(|i| format!("\t// Line {i} of running commentary.\n"))
            .collect::<String>();
        format!("package x\n\ntype S interface {{\n{prose}\n\tGet() error\n\tPut() error\n}}\n")
    };
    assert!(
        !rules_fired(&commented_iface(DENSITY_MIN_COMMENT_LINES)).contains(&"density"),
        "the floor is the largest count that stays silent"
    );
    assert!(
        rules_fired(&commented_iface(DENSITY_MIN_COMMENT_LINES + 1)).contains(&"density"),
        "one line past the floor must fire"
    );
}

/// The cap-only sentence, which no fixture reached until this test.
///
/// `every_instruction_opens_with_the_change_to_make` compares first words, and both arms open with
/// "shorten", so the suite could not tell them apart — while the cap carries the larger share of
/// real findings. A subject declaring no members is the only way to reach this arm.
#[test]
fn the_cap_alone_names_no_denominator() {
    let prose = (0..=ABSOLUTE_DOC_LINES)
        .map(|i| format!("// Line {i} of prose about the identifier below.\n"))
        .collect::<String>();
    let src = format!("package x\n\n{prose}type ID string\n");
    let message = findings_for(&src)
        .into_iter()
        .find(|f| f.rule == "docbloat")
        .map(|f| f.instruction)
        .expect("a 0-member subject over the cap fires docbloat");
    assert!(
        message.starts_with("shorten this doc comment:"),
        "AC8: {message}"
    );
    assert!(
        !message.contains("documenting"),
        "a subject with no members has no denominator to name: {message}"
    );
}
