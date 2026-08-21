//! Confirms pack concerns 2, 5, 6, 8, 9 and 11 against real parse trees at the pinned grammar
//! versions. Concerns 7 and 10 are not grammar facts and are not probed: marker stripping is
//! lexical and blank-line policy is a language convention, so both are pack decisions with nothing
//! for a parser to confirm. Rust is the one exception to 7, asserted under concern 6.
//!
//! Every assertion below is a pin check. When a grammar version moves, what fails here is the
//! mapping a pack was written against.
//!
//! **A claim about a grammar is asserted against the grammar's own node-kind table, never against
//! what a fixture happens to contain.** A fixture-derived set can only report kinds the fixture
//! exercises, so it confirms a claim that is too narrow instead of failing on it — and a claim that
//! was never asserted cannot be broken by a pin bump either. Fixtures are for shapes: fields,
//! positions, nesting.

use laconic_grammars::Grammar;
use std::collections::BTreeSet;
use tree_sitter::{Node, Tree};

fn source(g: Grammar) -> &'static str {
    g.probe_fixture().1
}

/// The comment node kinds a pack extracts for this grammar — concern 2.
fn comment_kinds(g: Grammar) -> &'static [&'static str] {
    match g {
        Grammar::Go | Grammar::Python => &["comment"],
        Grammar::Rust | Grammar::Java => &["line_comment", "block_comment"],
        Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => &["comment", "html_comment"],
    }
}

/// Doc kind for the grammars with no field to read — concern 5's marker-text half.
///
/// `starts_with("/**")` alone is wrong in a way that matters: it matches the empty comment `/**/`
/// and every `/****…****/` banner, which would make Doc kind out of exactly the input `banner`
/// exists to catch, costing that rule its gate tier and its Delete fix. Requiring a fourth
/// character that is neither `*` nor `/` excludes both. The known loss is `/***`-opened doc
/// comments, which no generator emits and which this trades away deliberately.
fn is_marker_doc(body: &str) -> bool {
    body.strip_prefix("/**")
        .is_some_and(|rest| !rest.starts_with(['*', '/']))
}

fn parse(g: Grammar) -> Tree {
    g.parser().parse(source(g), None).expect("fixture parses")
}

fn all_nodes(tree: &Tree) -> Vec<Node<'_>> {
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    let mut out = Vec::new();
    while let Some(n) = stack.pop() {
        out.push(n);
        for c in n.children(&mut cursor) {
            stack.push(c);
        }
    }
    out.sort_by_key(|n| n.start_byte());
    out
}

fn text<'a>(node: Node, src: &'a str) -> &'a str {
    &src[node.byte_range()]
}

/// The first node of `kind` whose `name` field is `name`.
fn find_named<'t>(tree: &'t Tree, kind: &str, name: &str, src: &str) -> Node<'t> {
    all_nodes(tree)
        .into_iter()
        .find(|n| {
            n.kind() == kind
                && n.child_by_field_name("name")
                    .is_some_and(|id| text(id, src) == name)
        })
        .unwrap_or_else(|| panic!("no `{kind}` named `{name}` in fixture"))
}

/// Statements in a subject's body, excluding comments — concern 9. The container is not always the
/// body node, so it is named per language.
fn statement_count(g: Grammar, kind: &str, name: &str, container_kind: &str) -> usize {
    let tree = parse(g);
    let src = source(g);
    let subject = find_named(&tree, kind, name, src);
    let body = subject.child_by_field_name("body").expect("body");
    let container = if body.kind() == container_kind {
        body
    } else {
        let mut cursor = body.walk();
        body.named_children(&mut cursor)
            .find(|n| n.kind() == container_kind)
            .unwrap_or_else(|| panic!("{}: no {container_kind} under body", g.name()))
    };
    let mut cursor = container.walk();
    container
        .named_children(&mut cursor)
        .filter(|n| !n.is_extra())
        .count()
}

/// Concern 2 — which node types carry comments, read off the grammar rather than off a fixture.
/// There is no type common to all seven grammars, which is why this cannot be an engine constant.
#[test]
fn comment_node_kinds_are_grammar_truth() {
    // Every named kind whose name contains "comment", and which of them a pack extracts. The two
    // differ only for Rust, where three of the five are children of a comment node rather than
    // comment nodes themselves.
    let expected: &[(Grammar, &[&str])] = &[
        (Grammar::Go, &["comment"]),
        (Grammar::Python, &["comment"]),
        (
            Grammar::Rust,
            &[
                "block_comment",
                "doc_comment",
                "inner_doc_comment_marker",
                "line_comment",
                "outer_doc_comment_marker",
            ],
        ),
        (Grammar::Java, &["block_comment", "line_comment"]),
        (Grammar::TypeScript, &["comment", "html_comment"]),
        (Grammar::Tsx, &["comment", "html_comment"]),
        (Grammar::JavaScript, &["comment", "html_comment"]),
    ];

    for (g, kinds) in expected {
        let declared: BTreeSet<&str> = g
            .named_node_kinds()
            .into_iter()
            .filter(|k| k.contains("comment"))
            .collect();
        let want: BTreeSet<&str> = kinds.iter().copied().collect();
        assert_eq!(declared, want, "comment-named node kinds for {}", g.name());

        // What a pack extracts must be a subset the grammar actually defines.
        let all: BTreeSet<&str> = g.named_node_kinds().into_iter().collect();
        for kind in comment_kinds(*g) {
            assert!(all.contains(kind), "{} does not define `{kind}`", g.name());
        }
    }
}

/// A kind declared for concern 2 that no fixture exercises is a kind nothing downstream is tested
/// against. This is the check that would have caught `html_comment` being omitted from the
/// JavaScript and TSX rows.
#[test]
fn every_declared_comment_kind_appears_in_its_fixture() {
    for g in Grammar::ALL {
        let tree = parse(g);
        let found: BTreeSet<&str> = all_nodes(&tree)
            .iter()
            .filter(|n| n.is_extra())
            .map(|n| n.kind())
            .collect();
        let want: BTreeSet<&str> = comment_kinds(g).iter().copied().collect();
        assert_eq!(
            found,
            want,
            "{} fixture exercises every declared kind",
            g.name()
        );
    }
}

/// A fixture that stops parsing cleanly means the pin moved under the mapping, not that the
/// fixture is wrong. Every assertion in this file rests on this one.
#[test]
fn every_fixture_parses_without_error_nodes() {
    for g in Grammar::ALL {
        let tree = parse(g);
        let broken: Vec<_> = all_nodes(&tree)
            .iter()
            .filter(|n| n.is_error() || n.is_missing())
            .map(|n| n.kind())
            .collect();
        assert!(broken.is_empty(), "{} produced {:?}", g.name(), broken);
    }
}

/// The runtime and the grammars version independently — every grammar crate depends only on
/// `tree-sitter-language`, so Cargo reports no conflict when the parser ABI is what actually
/// differs. Three of the seven are a major ABI behind and the pinned runtime loads both.
#[test]
fn pinned_grammars_span_two_abi_versions() {
    let expected = [
        (Grammar::Go, 15),
        (Grammar::Python, 15),
        (Grammar::Rust, 15),
        (Grammar::Java, 14),
        (Grammar::TypeScript, 14),
        (Grammar::Tsx, 14),
        (Grammar::JavaScript, 15),
    ];
    for (g, abi) in expected {
        assert_eq!(g.language().abi_version(), abi, "abi for {}", g.name());
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Line,
    Block,
    Doc,
}

fn classify(g: Grammar, n: Node, src: &str) -> Kind {
    let body = text(n, src);
    match g {
        // Rust settles Doc structurally; the rest is node kind.
        Grammar::Rust => {
            if n.child_by_field_name("doc").is_some() {
                Kind::Doc
            } else if n.kind() == "block_comment" {
                Kind::Block
            } else {
                Kind::Line
            }
        }
        // Java: node kind separates Line from Block, marker text lifts Block to Doc.
        Grammar::Java => {
            if is_marker_doc(body) {
                Kind::Doc
            } else if n.kind() == "block_comment" {
                Kind::Block
            } else {
                Kind::Line
            }
        }
        // One node kind for both forms, so every distinction is the marker. An html_comment runs
        // to end of line, which makes it Line.
        _ => {
            if is_marker_doc(body) {
                Kind::Doc
            } else if body.starts_with("/*") {
                Kind::Block
            } else {
                Kind::Line
            }
        }
    }
}

/// Concern 5 — Line, Block or Doc, for all seven grammars. Node kind alone never settles it: for
/// the five grammars whose only comment type is `comment`, Line versus Block comes from the
/// opening marker, and Doc comes from concern 6's mechanism rather than from either.
#[test]
fn comment_kind_classification() {
    let expected: &[(Grammar, &[(&str, Kind)])] = &[
        (
            Grammar::Go,
            &[
                ("//go:build linux", Kind::Line),
                ("// Package probe", Kind::Line),
                ("/* a detached block comment */", Kind::Block),
            ],
        ),
        (
            Grammar::Rust,
            &[
                ("//! Inner doc", Kind::Doc),
                ("// a line comment", Kind::Line),
                ("/* a block comment */", Kind::Block),
                ("/// Documents", Kind::Doc),
                ("/** A block doc comment. */", Kind::Doc),
            ],
        ),
        (
            Grammar::Java,
            &[
                ("// a line comment", Kind::Line),
                ("/* a block comment */", Kind::Block),
                ("/**\n * Documents an exported class.", Kind::Doc),
            ],
        ),
        (
            Grammar::TypeScript,
            &[
                ("// a line comment", Kind::Line),
                ("/* a block comment */", Kind::Block),
                ("<!-- an html comment -->", Kind::Line),
                ("/** Documents an exported function. */", Kind::Doc),
            ],
        ),
        (
            Grammar::Tsx,
            &[
                ("// a line comment", Kind::Line),
                ("/* a block comment */", Kind::Block),
                ("<!-- an html comment -->", Kind::Line),
                ("/** Documents an exported component. */", Kind::Doc),
            ],
        ),
        (
            Grammar::JavaScript,
            &[
                ("// a line comment", Kind::Line),
                ("/* a block comment */", Kind::Block),
                ("<!-- an html comment -->", Kind::Line),
                ("/** Documents an exported function. */", Kind::Doc),
            ],
        ),
    ];

    for (g, cases) in expected {
        let tree = parse(*g);
        let src = source(*g);
        let comments: Vec<Node> = all_nodes(&tree)
            .into_iter()
            .filter(|n| comment_kinds(*g).contains(&n.kind()))
            .collect();
        for (prefix, want) in *cases {
            let node = comments
                .iter()
                .find(|n| text(**n, src).starts_with(prefix))
                .unwrap_or_else(|| panic!("{}: no comment starting {prefix:?}", g.name()));
            assert_eq!(&classify(*g, *node, src), want, "{} {prefix:?}", g.name());
        }
    }

    // Python has no block comment form at all: every `comment` node is a Line comment, and Doc is
    // a string rather than a comment node.
    let tree = parse(Grammar::Python);
    let src = source(Grammar::Python);
    for n in all_nodes(&tree).iter().filter(|n| n.kind() == "comment") {
        assert!(
            text(*n, src).starts_with('#'),
            "python comment is always Line"
        );
    }
}

/// The empty comment `/**/` and an all-asterisk banner both open with `/**`. Classifying either as
/// Doc would cost `banner` its gate tier and its Delete fix on exactly the input it exists to
/// catch, so the two mechanisms are asserted against both forms.
#[test]
fn empty_and_banner_block_comments_are_not_doc_comments() {
    // Rust: the grammar declines them — neither carries a `doc` field. Nothing lexical is needed.
    let tree = parse(Grammar::Rust);
    let src = source(Grammar::Rust);
    for body in ["/**/", "/*******/"] {
        let node = all_nodes(&tree)
            .into_iter()
            .find(|n| text(*n, src) == body)
            .unwrap_or_else(|| panic!("rust fixture has {body}"));
        assert_eq!(node.kind(), "block_comment");
        assert!(
            node.child_by_field_name("doc").is_none(),
            "rust grammar declines {body} as a doc comment"
        );
        assert_eq!(classify(Grammar::Rust, node, src), Kind::Block);
    }

    // TypeScript: no field exists, so the marker test carries it.
    let tree = parse(Grammar::TypeScript);
    let src = source(Grammar::TypeScript);
    for body in ["/**/", "/****************/"] {
        let node = all_nodes(&tree)
            .into_iter()
            .find(|n| text(*n, src) == body)
            .unwrap_or_else(|| panic!("typescript fixture has {body}"));
        assert_eq!(classify(Grammar::TypeScript, node, src), Kind::Block);
    }

    // The naive test these replace, kept as the record of why the strict one exists.
    assert!("/**/".starts_with("/**") && !is_marker_doc("/**/"));
    assert!("/****************/".starts_with("/**") && !is_marker_doc("/****************/"));
    assert!(is_marker_doc("/** Documents something. */"));
    assert!(is_marker_doc("/**\n * Documents something.\n */"));
}

/// Concern 6, Rust — the doc comment is a grammar field. Not marker text: a pack reading `///`
/// lexically classifies `////////` as Doc kind, which costs `banner` its gate tier and its Delete
/// fix on exactly the input it exists to catch.
#[test]
fn rust_doc_comment_is_a_field_on_an_ordinary_comment_node() {
    let tree = parse(Grammar::Rust);
    let src = source(Grammar::Rust);
    let comments: Vec<_> = all_nodes(&tree)
        .into_iter()
        .filter(|n| matches!(n.kind(), "line_comment" | "block_comment"))
        .collect();

    let docs = comments
        .iter()
        .filter(|n| n.child_by_field_name("doc").is_some())
        .count();
    assert_eq!(docs, 3, "fixture has three doc comments");

    // `//!` and `///` differ by field, not by text inspection. `/**` carries the same `outer`
    // field as `///`, so the field answers the question for both comment node kinds and a pack
    // never inspects the marker.
    let markers: Vec<(&str, &str)> = comments
        .iter()
        .filter_map(|n| {
            let field = if n.child_by_field_name("inner").is_some() {
                "inner"
            } else if n.child_by_field_name("outer").is_some() {
                "outer"
            } else {
                return None;
            };
            Some((field, &text(*n, src)[..3]))
        })
        .collect();
    assert_eq!(
        markers,
        vec![("inner", "//!"), ("outer", "///"), ("outer", "/**")]
    );

    // The `doc` child is the body with its markers already removed — concern 7, for free, and only
    // here.
    let outer = comments
        .iter()
        .find(|n| text(**n, src).starts_with("///"))
        .expect("/// doc comment");
    let body = outer.child_by_field_name("doc").unwrap();
    assert_eq!(text(body, src), " Documents an exported function.\n");

    // A plain comment carries no doc field and no content child; its body is the byte range less
    // the leading token.
    let plain = comments
        .iter()
        .find(|n| text(**n, src) == "// a line comment")
        .expect("plain line comment");
    assert!(plain.child_by_field_name("doc").is_none());
}

/// Concern 6, Java and TS/JS — marker text on an ordinary comment node, because the grammars
/// expose no field and no distinct type for a doc comment.
#[test]
fn java_and_ts_doc_comments_are_marker_text() {
    for g in [
        Grammar::Java,
        Grammar::TypeScript,
        Grammar::Tsx,
        Grammar::JavaScript,
    ] {
        let tree = parse(g);
        let src = source(g);
        let doc_kind = if g == Grammar::Java {
            "block_comment"
        } else {
            "comment"
        };
        let docs: Vec<_> = all_nodes(&tree)
            .into_iter()
            .filter(|n| n.kind() == doc_kind && is_marker_doc(text(*n, src)))
            .collect();
        assert!(!docs.is_empty(), "{} has a doc comment", g.name());
        for d in &docs {
            assert!(d.child_by_field_name("doc").is_none(), "{}", g.name());
            assert_eq!(
                d.named_child_count(),
                0,
                "{} doc comment is a leaf",
                g.name()
            );
        }
    }
}

/// Concern 6, Go — position. The doc comment is the comment ending on the line directly above the
/// declaration, and nothing lexical distinguishes it from any other `comment` node.
#[test]
fn go_doc_comment_is_positional() {
    let tree = parse(Grammar::Go);
    let src = source(Grammar::Go);
    let subject = find_named(&tree, "function_declaration", "Exported", src);
    let doc = subject
        .prev_sibling()
        .expect("a node precedes the declaration");
    assert_eq!(doc.kind(), "comment");
    assert_eq!(
        doc.end_position().row + 1,
        subject.start_position().row,
        "doc comment sits on the line directly above its subject"
    );
    assert!(text(doc, src).starts_with("// Exported"));

    // The detached block comment is the same node kind and is not a doc comment: what separates
    // them is the blank line, which is why this cannot be a lexical test.
    let block = all_nodes(&tree)
        .into_iter()
        .find(|n| text(*n, src).starts_with("/* a detached"))
        .expect("detached block comment");
    let next = block.next_sibling().expect("a node follows");
    assert!(next.start_position().row > block.end_position().row + 1);
}

/// Concern 6, Python — position, and the subject is the enclosing scope rather than a following
/// one. A docstring is `(expression_statement (string))` as the first statement of a module,
/// class or function body; the concrete `string` is a direct child, so the `expression` supertype
/// in `node-types.json` does not appear in the tree and needs no traversal.
#[test]
fn python_docstring_is_the_first_statement_of_its_scope() {
    let tree = parse(Grammar::Python);
    let src = source(Grammar::Python);

    let is_docstring = |scope: Node| -> Option<String> {
        let mut cursor = scope.walk();
        let first = scope.named_children(&mut cursor).find(|n| !n.is_extra())?;
        if first.kind() != "expression_statement" {
            return None;
        }
        let inner = first.named_child(0)?;
        (inner.kind() == "string").then(|| text(inner, src).to_string())
    };

    let module = tree.root_node();
    assert_eq!(
        is_docstring(module).as_deref(),
        Some("\"\"\"Module docstring.\"\"\""),
        "module docstring is the first non-comment statement, and the shebang comment is an \
         extra that does not displace it"
    );

    for (kind, name, want) in [
        (
            "function_definition",
            "exported",
            "\"\"\"Function docstring.\"\"\"",
        ),
        ("class_definition", "Thing", "\"\"\"Class docstring.\"\"\""),
    ] {
        let subject = find_named(&tree, kind, name, src);
        let body = subject.child_by_field_name("body").expect("body");
        assert_eq!(is_docstring(body).as_deref(), Some(want), "{name}");
    }

    // A string anywhere but first position is a string. This is the whole reason the mechanism is
    // positional rather than lexical.
    let unexported = find_named(&tree, "function_definition", "_unexported", src);
    let body = unexported.child_by_field_name("body").expect("body");
    assert_eq!(is_docstring(body), None);
}

/// Concern 8 — a subject's visibility. Four unrelated mechanisms across five languages, which is
/// why this is a strategy rather than a declaration.
#[test]
fn subject_visibility_mechanisms() {
    // Go: the first rune of the identifier.
    let tree = parse(Grammar::Go);
    let src = source(Grammar::Go);
    for (name, exported) in [("Exported", true), ("unexported", false)] {
        let f = find_named(&tree, "function_declaration", name, src);
        let ident = f.child_by_field_name("name").unwrap();
        let first = text(ident, src).chars().next().unwrap();
        assert_eq!(first.is_uppercase(), exported, "go {name}");
    }

    // Rust: a `visibility_modifier` child, read as text. Presence is not the fact — `pub`,
    // `pub(crate)` and `pub(self)` are one node kind and three different answers.
    let tree = parse(Grammar::Rust);
    let src = source(Grammar::Rust);
    for (name, vis) in [
        ("exported", Some("pub")),
        ("crate_visible", Some("pub(crate)")),
        ("module_private", Some("pub(self)")),
        ("private", None),
    ] {
        let f = find_named(&tree, "function_item", name, src);
        let mut cursor = f.walk();
        let found = f
            .named_children(&mut cursor)
            .find(|n| n.kind() == "visibility_modifier")
            .map(|n| text(n, src));
        assert_eq!(found, vis, "rust {name}");
    }

    // Java: a `modifiers` child containing the keyword.
    let tree = parse(Grammar::Java);
    let src = source(Grammar::Java);
    for (name, vis) in [("exported", "public"), ("hidden", "private")] {
        let m = find_named(&tree, "method_declaration", name, src);
        let mut cursor = m.walk();
        let modifiers = m
            .children(&mut cursor)
            .find(|n| n.kind() == "modifiers")
            .expect("modifiers");
        assert!(text(modifiers, src).contains(vis), "java {name}");
    }

    // TypeScript, TSX and JavaScript: an `export_statement` wrapping the declaration at file
    // scope. Three separate grammars answering it identically is what makes one pack over three
    // grammars viable.
    for g in [Grammar::TypeScript, Grammar::Tsx, Grammar::JavaScript] {
        let tree = parse(g);
        let src = source(g);
        let exported_name = if g == Grammar::Tsx {
            "Component"
        } else {
            "exported"
        };
        let exported = find_named(&tree, "function_declaration", exported_name, src);
        assert_eq!(
            exported.parent().unwrap().kind(),
            "export_statement",
            "{} export",
            g.name()
        );
        let hidden = find_named(&tree, "function_declaration", "hidden", src);
        assert_eq!(hidden.parent().unwrap().kind(), "program", "{}", g.name());
    }

    // A TypeScript class member is a second, unrelated question in the same language.
    let tree = parse(Grammar::TypeScript);
    let src = source(Grammar::TypeScript);
    let member = find_named(&tree, "method_definition", "visible", src);
    let mut cursor = member.walk();
    assert!(
        member
            .children(&mut cursor)
            .any(|n| n.kind() == "accessibility_modifier"),
        "ts class member carries an accessibility modifier"
    );

    // Python: a leading underscore, which is a convention rather than a language fact. This is the
    // weakest answer in the set and the reason concern 11 carries a reversal condition.
    let tree = parse(Grammar::Python);
    let src = source(Grammar::Python);
    for (name, public) in [("exported", true), ("_unexported", false)] {
        let f = find_named(&tree, "function_definition", name, src);
        let ident = f.child_by_field_name("name").unwrap();
        assert_eq!(!text(ident, src).starts_with('_'), public, "python {name}");
    }
}

/// Concern 9 — a subject's statement count, for `density`'s ratio. The container is not always the
/// body node: Go interposes a `statement_list` between `block` and its statements.
#[test]
fn statement_containers_and_counts() {
    let expected: &[(Grammar, &str, &str, &str, usize)] = &[
        (
            Grammar::Go,
            "function_declaration",
            "Exported",
            "statement_list",
            3,
        ),
        (Grammar::Rust, "function_item", "exported", "block", 3),
        (Grammar::Java, "method_declaration", "exported", "block", 3),
        (
            Grammar::TypeScript,
            "function_declaration",
            "exported",
            "statement_block",
            3,
        ),
        (
            Grammar::Tsx,
            "function_declaration",
            "Component",
            "statement_block",
            3,
        ),
        (
            Grammar::JavaScript,
            "function_declaration",
            "exported",
            "statement_block",
            3,
        ),
    ];

    for (g, kind, name, container_kind, count) in expected {
        assert_eq!(
            statement_count(*g, kind, name, container_kind),
            *count,
            "{} statement count",
            g.name()
        );
    }
}

/// Python is the one language whose doc comment sits **inside** the statement container, because a
/// docstring is an `expression_statement` rather than an `extra`. Filtering extras removes the doc
/// comment in every other language and does not here, so a pack that stops at "count the non-extra
/// children" gives Python a denominator one larger than the same function in any other language,
/// and `density` is systematically harder to trip there.
#[test]
fn python_statement_count_includes_its_docstring() {
    let python = statement_count(Grammar::Python, "function_definition", "exported", "block");
    let rust = statement_count(Grammar::Rust, "function_item", "exported", "block");
    assert_eq!(
        python,
        rust + 1,
        "the fixtures are the same shape, so the difference is the docstring alone"
    );

    // And it really is the docstring, rather than a fixture that drifted.
    let tree = parse(Grammar::Python);
    let src = source(Grammar::Python);
    let subject = find_named(&tree, "function_definition", "exported", src);
    let body = subject.child_by_field_name("body").expect("body");
    let mut cursor = body.walk();
    let first = body
        .named_children(&mut cursor)
        .find(|n| !n.is_extra())
        .expect("a first statement");
    assert_eq!(first.kind(), "expression_statement");
    assert_eq!(first.named_child(0).unwrap().kind(), "string");
}

/// Concern 11 — the file's declared symbols with their visibility, which `implInInterface` needs
/// and which concern 8 cannot answer: concern 8 describes one subject, this enumerates every
/// declaration a name in a doc comment might resolve to.
///
/// The enumeration spans declaration forms, not just functions. A private type is the commonest
/// name a public doc comment leaks, so a pack that enumerates functions alone resolves nothing for
/// exactly the case the rule exists to catch.
#[test]
fn file_declared_symbols_span_more_than_functions() {
    // Go: top-level declarations are named children of the root, and each form nests its name one
    // level down in a `_spec` node.
    let tree = parse(Grammar::Go);
    let src = source(Grammar::Go);
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut declared: Vec<(&str, &str, bool)> = Vec::new();
    for decl in root.named_children(&mut cursor) {
        let (spec_kind, form) = match decl.kind() {
            "function_declaration" => {
                let name = text(decl.child_by_field_name("name").unwrap(), src);
                declared.push(("func", name, name.starts_with(char::is_uppercase)));
                continue;
            }
            "type_declaration" => ("type_spec", "type"),
            "const_declaration" => ("const_spec", "const"),
            "var_declaration" => ("var_spec", "var"),
            _ => continue,
        };
        let mut inner = decl.walk();
        for spec in decl.named_children(&mut inner) {
            if spec.kind() != spec_kind {
                continue;
            }
            let name = text(spec.child_by_field_name("name").unwrap(), src);
            declared.push((form, name, name.starts_with(char::is_uppercase)));
        }
    }
    assert_eq!(
        declared,
        vec![
            ("const", "MaxDepth", true),
            ("var", "defaultName", false),
            ("type", "Config", true),
            ("type", "internalState", false),
            ("func", "Exported", true),
            ("func", "unexported", false),
        ]
    );

    // Java: declarations are not children of the root — a method lives in a `class_body` and a
    // class can nest inside another — so the enumeration is a walk rather than a sibling scan.
    let tree = parse(Grammar::Java);
    let src = source(Grammar::Java);
    let public = |n: Node| {
        let mut c = n.walk();
        n.children(&mut c)
            .find(|m| m.kind() == "modifiers")
            .is_some_and(|m| text(m, src).contains("public"))
    };
    let mut declared: Vec<(&str, &str, bool)> = all_nodes(&tree)
        .into_iter()
        .filter_map(|n| {
            let form = match n.kind() {
                "class_declaration" => "class",
                "method_declaration" => "method",
                _ => return None,
            };
            let name = text(n.child_by_field_name("name")?, src);
            Some((form, name, public(n)))
        })
        .collect();
    declared.sort();
    assert_eq!(
        declared,
        vec![
            ("class", "Helper", false),
            ("class", "Probe", true),
            ("method", "exported", true),
            ("method", "hidden", false),
        ]
    );
}

/// Concern 11 across the three languages the Go/Java pair does not represent, because decision 23's
/// reversal condition is stated in terms of how many packs need a heuristic: "if two or more packs
/// cannot answer concern 11 without heuristics, the cheaper ruling is the design's wider version".
///
/// Evaluated here, the count is one. Rust reads a `visibility_modifier` and TypeScript reads an
/// `export_statement` parent — both language facts. Python has neither, and answers with the
/// leading-underscore convention. **One pack, so the condition is not met and the narrow ruling
/// stands.** It is also the one row a future language could tip.
#[test]
fn concern_eleven_needs_a_heuristic_in_exactly_one_language() {
    // Rust: a language fact, but the fact is the modifier's **text**. Presence alone is wrong —
    // `pub(self)` and `pub(in path)` are `visibility_modifier` nodes on strictly private items,
    // which is exactly the name `implInInterface` exists to catch.
    let tree = parse(Grammar::Rust);
    let src = source(Grammar::Rust);
    let visibility = |n: Node| -> Option<&str> {
        let mut c = n.walk();
        n.named_children(&mut c)
            .find(|m| m.kind() == "visibility_modifier")
            .map(|m| text(m, src))
    };
    let mut declared: Vec<(&str, &str, Option<&str>)> = all_nodes(&tree)
        .into_iter()
        .filter_map(|n| {
            let form = match n.kind() {
                "function_item" => "fn",
                "struct_item" => "struct",
                _ => return None,
            };
            Some((
                form,
                text(n.child_by_field_name("name")?, src),
                visibility(n),
            ))
        })
        .collect();
    declared.sort();
    assert_eq!(
        declared,
        vec![
            ("fn", "crate_visible", Some("pub(crate)")),
            ("fn", "exported", Some("pub")),
            ("fn", "module_private", Some("pub(self)")),
            ("fn", "private", None),
            ("struct", "Config", Some("pub")),
            ("struct", "Internal", None),
        ]
    );

    // TypeScript: a language fact. Every exported declaration has an `export_statement` parent,
    // whatever the declaration form.
    let tree = parse(Grammar::TypeScript);
    let src = source(Grammar::TypeScript);
    let mut declared: Vec<(&str, &str, bool)> = all_nodes(&tree)
        .into_iter()
        .filter_map(|n| {
            let form = match n.kind() {
                "function_declaration" => "function",
                "class_declaration" => "class",
                "interface_declaration" => "interface",
                "type_alias_declaration" => "type",
                _ => return None,
            };
            let exported = n.parent().is_some_and(|p| p.kind() == "export_statement");
            Some((form, text(n.child_by_field_name("name")?, src), exported))
        })
        .collect();
    declared.sort();
    assert_eq!(
        declared,
        vec![
            ("class", "Thing", true),
            ("function", "exported", true),
            ("function", "hidden", false),
            ("interface", "Shape", true),
            ("type", "Local", false),
        ]
    );

    // Python: no visibility construct exists in the grammar at all — asserted against the node-kind
    // table, not against the fixture, because the fixture can only show what it happens to use.
    // Every module-level binding is reachable by any importer, so the only available answer is the
    // leading underscore: a convention a file can ignore without becoming invalid.
    let visibility_kinds: Vec<&str> = Grammar::Python
        .named_node_kinds()
        .into_iter()
        .filter(|k| k.contains("visibility") || k.contains("access") || k.contains("modifier"))
        .collect();
    assert!(
        visibility_kinds.is_empty(),
        "python grammar exposes no visibility construct, found {visibility_kinds:?}"
    );

    // The other four all do, which is what makes Python's row the heuristic one rather than a gap
    // in the probe.
    for (g, kind) in [
        (Grammar::Rust, "visibility_modifier"),
        (Grammar::Java, "modifiers"),
        (Grammar::TypeScript, "accessibility_modifier"),
    ] {
        assert!(
            g.named_node_kinds().contains(&kind),
            "{} defines `{kind}`",
            g.name()
        );
    }

    let tree = parse(Grammar::Python);
    let src = source(Grammar::Python);
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut declared: Vec<(&str, &str, bool)> = Vec::new();
    for decl in root.named_children(&mut cursor) {
        let (form, name) = match decl.kind() {
            "function_definition" => ("def", text(decl.child_by_field_name("name").unwrap(), src)),
            "class_definition" => (
                "class",
                text(decl.child_by_field_name("name").unwrap(), src),
            ),
            "expression_statement" => {
                let Some(assign) = decl.named_child(0).filter(|n| n.kind() == "assignment") else {
                    continue;
                };
                (
                    "binding",
                    text(assign.child_by_field_name("left").unwrap(), src),
                )
            }
            _ => continue,
        };
        declared.push((form, name, !name.starts_with('_')));
    }
    assert_eq!(
        declared,
        vec![
            ("binding", "MAX_DEPTH", true),
            ("binding", "_default_name", false),
            ("def", "exported", true),
            ("def", "_unexported", false),
            ("class", "Thing", true),
        ]
    );
}
