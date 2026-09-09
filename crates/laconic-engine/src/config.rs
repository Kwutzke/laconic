//! `laconic.toml`: the file, its validation, and the per-language view a run resolves from it.
//!
//! Every error here aborts before a single file is read. A config the tool half understands
//! silently disables rules its author believed were on, which is the one failure worth an exit code
//! of its own.

use crate::registry::{Registry, Tier};
use crate::rules::structural::{ABSOLUTE_DOC_LINES, DENSITY_MAX_RATIO, DOC_LINES_PER_MEMBER};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// The three numeric thresholds, and the reason they are a struct rather than constants.
///
/// A language override can retune one for a single pack, so a threshold is a function of the file's
/// language and cannot be resolved once for the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub absolute_doc_lines: usize,
    pub doc_lines_per_member: usize,
    pub density_max_ratio: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            absolute_doc_lines: ABSOLUTE_DOC_LINES,
            doc_lines_per_member: DOC_LINES_PER_MEMBER,
            density_max_ratio: DENSITY_MAX_RATIO,
        }
    }
}

/// What one file's language runs with: the registry and thresholds after that language's overrides.
///
/// One value rather than two parameters because the two are resolved together and are wrong apart —
/// a caller holding a language's registry and the run's thresholds has silently dropped half of
/// every override.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub registry: Registry,
    pub thresholds: Thresholds,
}

impl From<Registry> for Resolved {
    fn from(registry: Registry) -> Self {
        Self {
            registry,
            thresholds: Thresholds::default(),
        }
    }
}

/// What one `[rules.<id>]` table may say.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleConfig {
    pub enabled: Option<bool>,
    /// One tier for the whole rule, applied to every kind the rule already applies to.
    ///
    /// The registry's dispositions are per kind — `narration` gates a line comment and warns on a
    /// doc comment — and this key cannot express that, so setting it flattens the distinction.
    /// There is deliberately no per-kind channel: a rule treating a doc comment differently is the
    /// registry's ruling, not a repository's. The consequence worth knowing is that a config cannot
    /// restate such a rule's defaults, which is why `defaults_toml` never emits this key.
    pub tier: Option<TierName>,
    pub autofix: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TierName {
    Gate,
    Warn,
}

impl From<TierName> for Tier {
    fn from(name: TierName) -> Self {
        match name {
            TierName::Gate => Tier::Gate,
            TierName::Warn => Tier::Warn,
        }
    }
}

/// Thresholds as the file may state them: each optional, so an unset one keeps its default rather
/// than resetting to zero.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdConfig {
    pub absolute_doc_lines: Option<usize>,
    pub doc_lines_per_member: Option<usize>,
    pub density_max_ratio: Option<f64>,
}

impl ThresholdConfig {
    fn apply(&self, base: Thresholds) -> Thresholds {
        Thresholds {
            absolute_doc_lines: self.absolute_doc_lines.unwrap_or(base.absolute_doc_lines),
            doc_lines_per_member: self
                .doc_lines_per_member
                .unwrap_or(base.doc_lines_per_member),
            density_max_ratio: self.density_max_ratio.unwrap_or(base.density_max_ratio),
        }
    }
}

/// A per-language block: the same knobs, scoped to one pack.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    #[serde(default)]
    pub rules: BTreeMap<String, RuleConfig>,
    #[serde(default)]
    pub thresholds: ThresholdConfig,
}

/// `laconic.toml` as written.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Replaces the default set rather than extending it — the fixture suite lives under a default
    /// exclusion, so extension alone would leave it unable to scan itself.
    pub excluded_paths: Option<Vec<String>>,
    #[serde(default)]
    pub rules: BTreeMap<String, RuleConfig>,
    #[serde(default)]
    pub thresholds: ThresholdConfig,
    /// Keyed by pack name. A key no pack claims is an error, not a no-op.
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageConfig>,
}

/// Why a run stopped before reading any file. Every variant exits 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Unreadable { path: PathBuf, message: String },
    Malformed { path: PathBuf, message: String },
    UnknownRule { id: String, scope: String },
    UnknownLanguage { name: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, message } => {
                write!(f, "{}: cannot be read: {message}", path.display())
            }
            Self::Malformed { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            Self::UnknownRule { id, scope } => write!(
                f,
                "unknown rule {id:?} in {scope} — remove it or correct the id"
            ),
            Self::UnknownLanguage { name } => write!(
                f,
                "no pack claims language {name:?} — remove the override or correct the name"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl ConfigFile {
    /// Parse one file. Absent is not this function's business: a missing config is a default run,
    /// and the caller distinguishes the two.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Unreadable {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        toml::from_str(&text).map_err(|e| ConfigError::Malformed {
            path: path.to_path_buf(),
            message: e.message().to_string(),
        })
    }

    /// Every rule id and language name the file names, checked against what exists.
    ///
    /// Runs before any file is read, and reports every problem rather than the first: a config with
    /// three typos should take one round trip to fix, not three.
    pub fn validate(&self, languages: &[&str]) -> Result<(), Vec<ConfigError>> {
        let registry = Registry::default();
        let mut errors = Vec::new();

        for id in self.rules.keys() {
            if registry.get(id).is_none() {
                errors.push(ConfigError::UnknownRule {
                    id: id.clone(),
                    scope: "[rules]".to_string(),
                });
            }
        }
        for (name, language) in &self.languages {
            if !languages.contains(&name.as_str()) {
                errors.push(ConfigError::UnknownLanguage { name: name.clone() });
            }
            for id in language.rules.keys() {
                if registry.get(id).is_none() {
                    errors.push(ConfigError::UnknownRule {
                        id: id.clone(),
                        scope: format!("[languages.{name}.rules]"),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// The registry and thresholds one language runs with: the global layer, then that language's.
    ///
    /// Callers must validate first — an unknown id is silently skipped here, because reporting it
    /// per file would report it once per file.
    pub fn resolve(&self, language: &str) -> Resolved {
        let mut registry = Registry::default();
        let mut thresholds = self.thresholds.apply(Thresholds::default());

        apply_rules(&mut registry, &self.rules);
        if let Some(overrides) = self.languages.get(language) {
            thresholds = overrides.thresholds.apply(thresholds);
            apply_rules(&mut registry, &overrides.rules);
        }
        Resolved {
            registry,
            thresholds,
        }
    }

    /// The excluded-path set, replaced wholesale when the file states one.
    pub fn excluded_paths(&self) -> Option<Vec<String>> {
        self.excluded_paths.clone()
    }
}

fn apply_rules(registry: &mut Registry, rules: &BTreeMap<String, RuleConfig>) {
    for (id, config) in rules {
        if let Some(enabled) = config.enabled {
            let _ = registry.set_enabled(id, enabled);
        }
        if let Some(tier) = config.tier {
            let _ = registry.set_tier(id, tier.into());
        }
        if let Some(autofix) = config.autofix {
            let _ = registry.set_autofix(id, autofix);
        }
    }
}

/// The nearest `laconic.toml` at or above `start`.
///
/// Nearest rather than merged: two config files on one path would make a rule's disposition depend
/// on where the run was invoked from.
pub fn discover(start: &Path) -> Option<PathBuf> {
    // Absolute before walking, because `parent()` is lexical: `Path::new(".").parent()` is `Some("")`
    // and `""`'s is `None`, so a relative start tested its own directory twice and stopped there —
    // reporting no config while one sat in the parent, which is the silent half-understood-config
    // outcome this module opens by refusing.
    // `absolute`, not `canonicalize`: this needs the path rooted so the walk terminates at the
    // filesystem root, and nothing more. Canonicalising would also resolve symlinks, which changes
    // which `laconic.toml` a symlinked checkout finds and what path the caller is handed back.
    let owned;
    let start = match std::path::absolute(start) {
        Ok(rooted) => {
            owned = rooted;
            owned.as_path()
        }
        Err(_) => start,
    };
    let mut dir = Some(start);
    while let Some(current) = dir {
        let candidate = current.join("laconic.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

/// The defaults as a `laconic.toml` would state them, rendered from the registry rather than
/// transcribed: a hand-written table of fifteen rules goes stale on the first re-tiering, in the
/// one file whose whole job is telling a reader what the defaults are.
///
/// **Tier is documented per kind and settable only per rule**, because a rule's tier can differ by
/// comment kind while a config's `tier` key re-tiers every kind at once.
pub fn defaults_toml() -> String {
    use crate::registry::default_rules;
    use std::fmt::Write as _;

    let t = Thresholds::default();
    let mut out = String::new();
    out.push_str(DEFAULTS_PREAMBLE);

    let _ = writeln!(out, "[thresholds]");
    let _ = writeln!(out, "absolute_doc_lines = {}", t.absolute_doc_lines);
    let _ = writeln!(out, "doc_lines_per_member = {}", t.doc_lines_per_member);
    let _ = writeln!(out, "density_max_ratio = {}", t.density_max_ratio);

    for entry in default_rules() {
        let _ = writeln!(
            out,
            "\n# line: {}   block: {}   doc: {}",
            kind_str(entry.line),
            kind_str(entry.block),
            kind_str(entry.doc),
        );
        let _ = writeln!(out, "[rules.{}]", entry.id);
        let _ = writeln!(out, "enabled = {}", entry.enabled);
        let _ = writeln!(out, "autofix = {}", entry.autofix);
    }
    out
}

const DEFAULTS_PREAMBLE: &str = "\
# laconic's shipped defaults, stated in full.
#
# Every value below is the default laconic would have used anyway, so a run against this section
# alone is a run with no laconic.toml at all. Change one to change that rule for this repository;
# delete a block to leave it alone. Anything a file adds *outside* this section — an
# `excluded_paths` list, a `[languages.…]` block — is a real change, and the identity stops there.
#
# Each rule's comment gives the disposition every comment kind gets as tier/fix, or `-` where the
# rule does not apply to that kind. A `tier` key re-tiers every kind the rule applies to; there is
# no per-kind key, because a rule that treats a doc comment differently is the registry's ruling
# and not a repository's.
#
# gate findings decide exit status: any unsuppressed one exits 1. A delete fix is the only shape
# `laconic fix` applies, and only where autofix is on.

";

fn kind_str(disposition: Option<crate::registry::Disposition>) -> String {
    match disposition {
        Some(d) => format!(
            "{}/{}",
            crate::report::tier_str(d.tier),
            crate::report::fix_str(d.fix)
        ),
        None => "-".to_string(),
    }
}
