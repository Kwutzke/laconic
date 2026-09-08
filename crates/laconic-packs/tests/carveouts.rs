//! AC3: every carve-out has a negative test, and **each test fails when its carve-out is removed**.
//!
//! The directive text comes from a hand-written fixture, never from the pack's own prefix list.
//! Generating it from the list makes the test circular — removing any prefix changes the result,
//! including a prefix that names nothing real — and a circular test cannot fail. Measured: adding
//! `"zzz-inert"` to a pack passed an earlier version of this file.
//!
//! So the fixture and the list are cross-checked against each other:
//!
//! 1. Every prefix a pack declares is **exercised by a line in that pack's fixture**. A prefix
//!    nobody wrote a directive for fails here, which is what catches an invented carve-out.
//! 2. The fixture reports **no findings** as the pack ships.
//! 3. With any one prefix removed, the fixture reports **at least one finding**. A carve-out that
//!    protects nothing fails here.

use laconic_engine::domain::{CommentKind, DeclaredSymbol, Visibility};
use laconic_engine::pack::{BlankLinePolicy, DocComment, Pack};
use laconic_engine::{
    Config, Resolved, Rules, all_block_rules, all_subject_rules, analyse, dispatch, resolve,
};
use laconic_grammars::Grammar;
use laconic_packs::all;
use std::path::Path;
use tree_sitter::Node;

/// A pack with exactly one machine-directive prefix removed.
struct WithoutCarveOut {
    inner: Box<dyn Pack>,
    remaining: Vec<&'static str>,
}

impl WithoutCarveOut {
    fn new(inner: Box<dyn Pack>, dropped: &str) -> Self {
        let remaining = inner
            .machine_directive_prefixes()
            .iter()
            .copied()
            .filter(|p| *p != dropped)
            .collect();
        Self { inner, remaining }
    }
}

impl Pack for WithoutCarveOut {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn extensions(&self) -> &[(&'static str, Grammar)] {
        self.inner.extensions()
    }
    fn comment_node_kinds(&self, grammar: Grammar) -> &[&'static str] {
        self.inner.comment_node_kinds(grammar)
    }
    fn machine_directive_prefixes(&self) -> &[&'static str] {
        &self.remaining
    }
    fn generated_file_markers(&self) -> &[&'static str] {
        self.inner.generated_file_markers()
    }
    fn comment_kind(&self, node: Node, src: &str) -> CommentKind {
        self.inner.comment_kind(node, src)
    }
    fn doc_comments<'t>(&self, root: Node<'t>, src: &str) -> Vec<DocComment<'t>> {
        self.inner.doc_comments(root, src)
    }
    fn subject_nodes<'t>(&self, root: Node<'t>) -> Vec<Node<'t>> {
        self.inner.subject_nodes(root)
    }
    fn comment_body(&self, node: Node, src: &str) -> String {
        self.inner.comment_body(node, src)
    }
    fn visibility(&self, subject: Node, src: &str) -> Visibility {
        self.inner.visibility(subject, src)
    }
    fn member_count(&self, subject: Node, src: &str) -> usize {
        self.inner.member_count(subject, src)
    }
    fn blank_line_policy(&self) -> BlankLinePolicy {
        self.inner.blank_line_policy()
    }
    fn declared_symbols(&self, root: Node, src: &str) -> Vec<DeclaredSymbol> {
        self.inner.declared_symbols(root, src)
    }
}

fn pack_for(ext: &str) -> Box<dyn Pack> {
    let idx = all()
        .iter()
        .position(|p| p.extensions().iter().any(|(e, _)| *e == ext))
        .unwrap_or_else(|| panic!("no pack claims .{ext}"));
    all()
        .into_iter()
        .nth(idx)
        .expect("index came from the same list")
}

fn grammar_for(ext: &str) -> Grammar {
    let packs = all();
    let path_string = format!("x.{ext}");
    resolve(&packs, Path::new(&path_string))
        .unwrap_or_else(|| panic!("no pack claims .{ext}"))
        .1
}

/// Hand-written, one per pack. The directives here are the ones real tools emit; nothing in this
/// list is derived from the pack's own declarations.
fn fixture(ext: &str) -> (&'static str, &'static str) {
    match ext {
        "go" => ("carveouts.go", include_str!("../testdata/go/carveouts.go")),
        "py" => (
            "carveouts.py",
            include_str!("../testdata/python/carveouts.py"),
        ),
        "rs" => (
            "carveouts.rs",
            include_str!("../testdata/rust/carveouts.rs"),
        ),
        "java" => (
            "carveouts.java",
            include_str!("../testdata/java/carveouts.java"),
        ),
        "ts" => (
            "carveouts.ts",
            include_str!("../testdata/typescript/carveouts.ts"),
        ),
        other => panic!("no carve-out fixture for .{other}"),
    }
}

fn findings_on(pack: &dyn Pack, ext: &str) -> Vec<String> {
    let (name, src) = fixture(ext);
    let path = Path::new(name);
    let analysis = analyse(pack, grammar_for(ext), path, src, &Config::unrestricted())
        .expect("a carve-out fixture is never excluded whole");
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let (findings, _) = dispatch(path, src, &analysis, &Resolved::default(), &rules);
    findings
        .into_iter()
        .filter(|f| !f.suppressed)
        .map(|f| format!("{}:{}", f.line, f.rule))
        .collect()
}

/// 1. The fixture exercises every prefix the pack declares.
fn every_prefix_is_exercised(ext: &str) {
    let pack = pack_for(ext);
    let (name, src) = fixture(ext);
    let unexercised: Vec<&str> = pack
        .machine_directive_prefixes()
        .iter()
        .copied()
        .filter(|prefix| !src.contains(*prefix))
        .collect();
    assert!(
        unexercised.is_empty(),
        "{name} exercises no directive for {unexercised:?} — a carve-out with no fixture line is a \
         carve-out nobody checked"
    );
}

/// 2 and 3. Clean as shipped; not clean with any one carve-out removed.
fn each_carve_out_does_work(ext: &str) {
    let shipped = pack_for(ext);
    let clean = findings_on(shipped.as_ref(), ext);
    assert!(
        clean.is_empty(),
        ".{ext} carve-out fixture reports findings as shipped: {clean:?}"
    );

    let mut inert = Vec::new();
    for prefix in shipped.machine_directive_prefixes().iter().copied() {
        let without = WithoutCarveOut::new(pack_for(ext), prefix);
        if findings_on(&without, ext).is_empty() {
            inert.push(prefix);
        }
    }
    assert!(
        inert.is_empty(),
        ".{ext}: removing {inert:?} changed nothing, so those carve-outs protect nothing and their \
         tests prove nothing"
    );
}

#[test]
fn go_carve_outs() {
    every_prefix_is_exercised("go");
    each_carve_out_does_work("go");
}

/// The Python fixture carries both PEP 263 encoding forms. `# -*- coding: utf-8 -*-` is caught by
/// the `-*-` prefix, so the bare `# coding: utf-8` is the only line the `coding:` carve-out is the
/// sole protection for — without it that prefix tests as inert.
///
/// The fixture also carries no prose. A comment explaining a directive is itself a detached
/// comment, and this is a linter for comments: the explanation belongs here, not there.
#[test]
fn python_carve_outs() {
    every_prefix_is_exercised("py");
    each_carve_out_does_work("py");
}

#[test]
fn rust_carve_outs() {
    every_prefix_is_exercised("rs");
    each_carve_out_does_work("rs");
}

#[test]
fn java_carve_outs() {
    every_prefix_is_exercised("java");
    each_carve_out_does_work("java");
}

#[test]
fn typescript_carve_outs() {
    every_prefix_is_exercised("ts");
    each_carve_out_does_work("ts");
}

/// A generated file is excluded whole, and the same standard applies: without the marker the file
/// is analysed normally.
#[test]
fn generated_markers_exclude_the_file() {
    for ext in ["go", "py", "rs", "java", "ts"] {
        let pack = pack_for(ext);
        let (name, src) = fixture(ext);
        let path = Path::new(name);
        let comment = if ext == "py" { "#" } else { "//" };
        for marker in pack.generated_file_markers() {
            let generated = format!("{comment} {marker} by a tool\n{src}");
            assert!(
                analyse(
                    pack.as_ref(),
                    grammar_for(ext),
                    path,
                    &generated,
                    &Config::unrestricted()
                )
                .is_err(),
                ".{ext} {marker:?}: a generated file is excluded whole"
            );
        }
        assert!(
            analyse(
                pack.as_ref(),
                grammar_for(ext),
                path,
                src,
                &Config::unrestricted()
            )
            .is_ok(),
            ".{ext}: a file without a marker is analysed"
        );
    }
}

/// `// rustfmt::skip` does not exist — rustfmt's skip is the `#[rustfmt::skip]` attribute, and an
/// attribute is not a comment. Carving out a comment form no tool emits silently exempts real
/// comments that happen to start that way.
#[test]
fn rust_does_not_carve_out_a_comment_form_that_does_not_exist() {
    assert!(
        !pack_for("rs")
            .machine_directive_prefixes()
            .iter()
            .any(|p| p.contains("rustfmt")),
        "rustfmt has no comment-based skip directive"
    );
}
