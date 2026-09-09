# laconic

Finds and deletes comments that carry no information, in any language it has a pack for, with no
model in the loop.

```
$ laconic check .
service.go:267:3: gate [restate] remove this comment: it names only what the code it is attached to already says
fix.rs:49:1: warn [docbloat] shorten this doc comment: 10 lines — keep only what a reader cannot derive; the rest belongs in a document if it belongs anywhere
```

Diagnostics are instructions describing the change — `remove this comment` — rather than labels
describing the code, because the consumer that shaped every other decision is an agent acting on one
diagnostic with no surrounding context.

## Install

```sh
cargo install --path crates/laconic-cli
```

One self-contained binary, no runtime to install. Rust 1.85 or newer; the tree-sitter grammars are C
sources that `cc` compiles during the build, so there is no FFI layer to maintain and nothing to
install alongside.

## Use

```
laconic check [PATH...]     report findings; exit 1 if any gates
laconic fix [PATH...]       apply every autofixable finding, then report what remains
laconic defaults            print the shipped defaults as a laconic.toml

  --since REF               scan the files changed since REF, whole; needs git
  --format human|machine    output shape (default: human)
  --config PATH             use this config file rather than discovering one
  --no-config               run on the shipped defaults, ignoring any laconic.toml
```

**Exit codes are the contract.** `0` clean, `1` a gate finding, `2` the run never started — a usage
error, or a config laconic could not fully understand. A tool that exits 0 on a config it half
understood is the failure `2` exists to prevent.

`--since` selects the files changed since a ref and reads **each one whole**, including the comments
on lines nobody touched. It is repository-wide rather than relative to the working directory, so
running it from a subdirectory still covers the branch. It cannot be combined with named paths.

`--format machine` is the shape to consume from anything that is not a person: one tab-separated
record per finding carrying the rule, file, byte span, line, column, tier and fix shape, then an
indented `instruction` line. The byte span is there so an agent applies a deletion without
re-deriving the range from the instruction text.

## The fifteen rules

Two independent axes. **Tier** decides exit status: any unsuppressed `gate` finding exits 1.
**Autofix** decides whether `laconic fix` touches the finding. A rule can gate without being
autofixed, and most do.

| Rule | Tier | What it reports |
|---|---|---|
| `narration` | gate | the comment narrates the edit rather than the code |
| `banner` | gate | a rule of repeated punctuation, or a standalone all-caps label |
| `restate` | gate | it names only what the code it is attached to already says |
| `commentedOutCode` | gate | commented-out code; version control already has it |
| `attribution` | gate | authorship that belongs in version control |
| `detached` | gate | a comment floating free of the code it describes |
| `fileref` | gate | a path reference that goes stale on the first rename |
| `ignoreReason` | gate | a `laconic:ignore` directive with no reason |
| `density` | warn | too much commentary against the code it covers |
| `docbloat` | warn | a doc comment longer than what a caller cannot derive |
| `implInInterface` | warn | an unexported name in a doc comment a caller cannot use |
| `hedging` | warn | a hedge standing in for the condition it hides |
| `vague` | warn | a term that names no behaviour, or a trailing `etc.` |
| `task` | warn | a TODO or FIXME with no tracker reference |
| `deadIgnore` | warn | a directive whose rule ran and did not fire |

**`laconic fix` applies `narration` and `banner` only.** The other four gate rules also have a
deletion as their fix shape but ship with autofix off: each has a stated false-positive mode, and
the friction is the price of not deleting good comments unattended. `fileref` and `ignoreReason`
are never autofixable at any setting — for `ignoreReason` the only deletion that makes it pass is
deleting the directive, which silently re-enables the rule it suppressed.

Thresholds are configurable: `absolute_doc_lines = 6`, `doc_lines_per_member = 3`,
`density_max_ratio = 0.2`. `absolute_doc_lines` is the owner's ruling against the corpus and
`doc_lines_per_member` is the specification's estimate; `density_max_ratio` is
comment lines per line of code, calibrated against a Go corpus and then set one step stricter, on
the view that an `laconic:ignore density — <reason>` on the subject that earns its commentary beats
a threshold permissive enough never to ask.

## Languages

Adding a language is adding a pack, not touching the engine.

| Pack | Extensions |
|---|---|
| Go | `.go` |
| Rust | `.rs` |
| Python | `.py` `.pyi` |
| Java | `.java` |
| TypeScript / JavaScript | `.ts` `.mts` `.cts` `.tsx` `.js` `.mjs` `.cjs` `.jsx` |

An extension no pack claims produces no output and is not an error — laconic runs over whole
repositories, and a line per `.json` would make the findings unreadable.

Comments come from the parse tree, never from a regex over raw bytes, which would match inside
string literals. Which node types carry comments is a pack's business: there is no node type common
to every grammar, and a language's doc comment may not be a comment node at all — Rust and Java
grammars emit `line_comment` and `block_comment`, and Python docstrings are string nodes.

## Configuration

`laconic defaults` prints the shipped defaults as a `laconic.toml`. A run against that output alone
is identical to a run with no config file, so the file is a starting point rather than a fork.

```toml
excluded_paths = ["testdata", "fixtures", "vendor", "node_modules"]

[rules.docbloat]
enabled = true
tier = "gate"
```

`excluded_paths` matches whole path **components** by equality: `vendor` excludes `a/vendor/b.go`,
while `src/generated` and `gen*` match no component and exclude nothing. Stating the key replaces
the four shipped defaults rather than extending them.

To keep a comment a rule objects to, say why on the record:

```go
// laconic:ignore docbloat — this is the contract an adapter author implements against
```

The reason is mandatory. A directive without one is itself a finding.

## Running it automatically

Findings are worth nothing unless something runs them without being asked. `docs/integrations.md`
carries the detail; all three consume the same binary, the same `laconic.toml` and the same exit
codes.

- **A Claude Code `Stop` hook** — `laconic check --since <default branch>` when an agent finishes a
  turn, blocking on a gate finding. Scoping to the branch is what makes this usable: on one private
  Go repository a whole-repository run reports 1352 gate findings that no current session caused,
  while across its last 40 merged branches under branch scope, 26 report nothing at all.
- **`hooks/pre-commit`** — the reference implementation of the staged-diff check. It lints the
  staged content via `git show :<path>` rather than the working tree, because a hook that blocks a
  commit over edits the author deliberately left unstaged is a hook that gets bypassed.
- **CI** — `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test`, then laconic over its own source. Gate findings fail the job; warn findings print.

If no binary is found, both hooks say so and allow the operation. A missing tool is not a finding,
and failing closed on one teaches people to bypass the check.

## What it will never do

**No model calls, ever.** Semantic judgment — "is this comment misleading?", "does this code need a
comment it lacks?" — is out of scope permanently, because it is not statically decidable. Both
belong to review rather than to a linter.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p laconic-cli -- check .
```

Every rule has positive and negative fixtures under `testdata/<lang>/<rule>/`, and every carve-out
in every pack has an explicit negative test — a rule that fires on a machine-parsed directive gets
the linter disabled on day one, so each carve-out is written before the rule it protects.

## Licence

MIT — see [`LICENSE`](LICENSE).
