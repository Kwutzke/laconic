//! Excluded regions, applied before any rule runs.

use crate::domain::CommentBlock;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    /// Path fragments whose files are skipped.
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
    /// The position test is the caller's: `attribution` must still fire on a mid-file notice,
    /// which is the case no deterministic test can separate from vanity.
    pub fn is_licence_header(&self, block: &CommentBlock) -> bool {
        let body = block.body().to_lowercase();
        ["copyright", "license", "licence", "spdx-license-identifier"]
            .iter()
            .any(|m| body.contains(m))
    }
}
