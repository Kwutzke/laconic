//! The pinned tree-sitter grammars, behind one enum.
//!
//! A pack selects a grammar per source extension rather than owning one: TypeScript, TSX and
//! JavaScript are three grammars behind a single TS/JS pack.

use tree_sitter::Language;

/// A grammar laconic can parse with. Exactly one parse tree shape per variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Grammar {
    Go,
    Python,
    Rust,
    Java,
    TypeScript,
    Tsx,
    JavaScript,
}

impl Grammar {
    /// Every variant, in the order the probe reports them.
    ///
    /// A grammar that is added and never probed is the failure this crate exists to prevent, so
    /// adding a variant must break the build here. `ALL_IS_EXHAUSTIVE` below is what does it.
    pub const ALL: [Grammar; 7] = [
        Grammar::Go,
        Grammar::Python,
        Grammar::Rust,
        Grammar::Java,
        Grammar::TypeScript,
        Grammar::Tsx,
        Grammar::JavaScript,
    ];

    /// Fails to compile when a variant is added without extending [`Grammar::ALL`]: the match arm
    /// is not exhaustive, and the array length no longer matches its declared size.
    const ALL_IS_EXHAUSTIVE: () = {
        const fn listed(g: Grammar) -> usize {
            match g {
                Grammar::Go => 0,
                Grammar::Python => 1,
                Grammar::Rust => 2,
                Grammar::Java => 3,
                Grammar::TypeScript => 4,
                Grammar::Tsx => 5,
                Grammar::JavaScript => 6,
            }
        }
        assert!(listed(Grammar::JavaScript) + 1 == Grammar::ALL.len());
    };

    pub fn name(self) -> &'static str {
        let () = Self::ALL_IS_EXHAUSTIVE;
        match self {
            Grammar::Go => "go",
            Grammar::Python => "python",
            Grammar::Rust => "rust",
            Grammar::Java => "java",
            Grammar::TypeScript => "typescript",
            Grammar::Tsx => "tsx",
            Grammar::JavaScript => "javascript",
        }
    }

    pub fn language(self) -> Language {
        match self {
            Grammar::Go => tree_sitter_go::LANGUAGE.into(),
            Grammar::Python => tree_sitter_python::LANGUAGE.into(),
            Grammar::Rust => tree_sitter_rust::LANGUAGE.into(),
            Grammar::Java => tree_sitter_java::LANGUAGE.into(),
            Grammar::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Grammar::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Grammar::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        }
    }

    /// The probe fixture for this grammar, and its file name.
    ///
    /// Lives here rather than in the probe because the test suite and the dump example both need
    /// it, and a second copy of this map is a second list to forget a grammar in.
    pub fn probe_fixture(self) -> (&'static str, &'static str) {
        match self {
            Grammar::Go => ("go.go", include_str!("../probe/go.go")),
            Grammar::Python => ("python.py", include_str!("../probe/python.py")),
            Grammar::Rust => ("rust.rs", include_str!("../probe/rust.rs")),
            Grammar::Java => ("java.java", include_str!("../probe/java.java")),
            Grammar::TypeScript => ("typescript.ts", include_str!("../probe/typescript.ts")),
            Grammar::Tsx => ("tsx.tsx", include_str!("../probe/tsx.tsx")),
            Grammar::JavaScript => ("javascript.js", include_str!("../probe/javascript.js")),
        }
    }

    pub fn parser(self) -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("pinned grammar ABI is incompatible with the pinned tree-sitter runtime");
        parser
    }

    /// Every named node kind this grammar defines. Grammar truth, independent of what any fixture
    /// happens to instantiate — a fixture-derived set can only confirm a claim that is too narrow.
    pub fn named_node_kinds(self) -> Vec<&'static str> {
        let lang = self.language();
        (0..lang.node_kind_count())
            .map(|id| id as u16)
            .filter(|id| lang.node_kind_is_named(*id))
            .filter_map(|id| lang.node_kind_for_id(id))
            .collect()
    }
}
