//! Excluded regions, applied before any rule runs.

use crate::domain::CommentBlock;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    /// Path **components** whose files are skipped — whole segments, compared for equality.
    ///
    /// `vendor` excludes `a/vendor/b.go`; `src/generated` and `gen*` match no component and
    /// exclude nothing.
    ///
    /// **Replaceable, not just extendable**, and that is a requirement rather than a convenience:
    /// AC2 puts every fixture under `testdata/<lang>/<rule>/`, so the fixture suite runs with this
    /// list cleared and would otherwise scan nothing at all.
    pub excluded_paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            excluded_paths: ["testdata", "fixtures", "vendor", "node_modules"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

impl Config {
    /// Nothing excluded — what the fixture suite runs with.
    pub fn unrestricted() -> Self {
        Self {
            excluded_paths: Vec::new(),
        }
    }

    pub fn is_excluded_path(&self, path: &Path) -> bool {
        path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| self.excluded_paths.iter().any(|e| e == s))
        })
    }

    /// A licence or copyright notice, which is carved out when it sits at the top of a file.
    ///
    /// The marker must **open a line**. Matching anywhere in the body deletes any first block that
    /// merely mentions a licence — and the commonest first block in a Go file is the package doc
    /// comment, so `// Package x implements the MIT-licensed parser.` would vanish whole, taking a
    /// pkg.go.dev surface with it.
    ///
    /// The position test is the caller's: `attribution` must still fire on a mid-file notice,
    /// which is the case no deterministic test can separate from vanity.
    pub fn is_licence_header(&self, block: &CommentBlock) -> bool {
        const MARKERS: &[&str] = &[
            "copyright",
            "licensed under",
            "licence",
            "license",
            "spdx-license-identifier",
            "all rights reserved",
        ];
        block.body().lines().any(|line| {
            let line = line.trim_start().to_lowercase();
            MARKERS.iter().any(|m| line.starts_with(m))
        })
    }
}
