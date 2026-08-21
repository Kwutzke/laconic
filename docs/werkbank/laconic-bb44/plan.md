# laconic v1 — plan

Mode: argumentative. Tracks `laconic#bb44`, against the design at `design.md` in this directory.

This document holds the approach: what order the work goes in and what constraint produced that
order, what exploration found that the design did not know, the facts checked this session with where
each was checked, and what test coverage exists. **The work items are kata subtasks under
`laconic#bb44` and are not reproduced here** — two lists of the same work drift, and kata is the one
that stays correct. `/finish` deletes this file.

## Contents

- 1. Approach and sequencing
- 2. Traps
- 3. Verified facts
- 4. Which existing tests cover this area

## 1. Approach and sequencing

The order is: toolchain and grammar facts, the two interfaces, the pipeline's output half, the rules
split by what they consume, the remaining packs, `fix`, the surface, acceptance. Each transition is
forced by something, and the forcing constraint is the part worth reading.

**Grammar facts come before anything that consumes them.** Two constraints stacked here. C9: no
toolchain, so nothing builds. And the design's per-language mechanisms were intent against parsers
nobody had run. Both are discharged — the toolchain is installed and the probe suite in
`laconic-grammars` confirms concerns 2, 5, 6, 8, 9 and 11 at the pins, with the table on
`laconic#bb44`. **Every pack subtask reads that table rather than re-deriving it, and the probe is
the pin check C10 requires: when a grammar version moves, what fails is the mapping a pack was
written against.**

**The pack trait and the engine pipeline are built together, with Go as the only pack.** A trait with
no implementation is a guess about what an implementation needs. Go is the choice because its doc
comment is positional, which is the mechanism least likely to let a positional assumption hide inside
the engine.

**The registry, dispatch, reconcile and reporter land before any rule.** A rule with nowhere to
report is untestable, and the reporter is a contract (design §5) rather than formatting, so it cannot
be retrofitted around rules already written against an ad-hoc shape.

**Rules split by what they consume, not by tier.** Seven need only the stripped comment body (pack
concern 7). Six need subject facts, attachment, and the §7 ERROR-node suppression partition. The
remaining two, `ignoreReason` and `deadIgnore`, consume neither — they read surviving ignore
directives at reconcile — and they ship with the second group, because that is the subtask where the
reconcile stage is first exercised by a rule. The first line is where a reviewer can accept the
deny-list matching and reject the subject handling, which is the test for whether a split is real.

**The four remaining packs come after the rules.** AC3 requires each carve-out's negative test to
fail when its carve-out is removed. A test cannot fail that way while the rule it guards against does
not run.

**`fix` follows the packs, not just the rules that produce Delete findings.** AC5 requires blank-line
handling around each removed block to match the language's convention, and that policy is pack
concern 10 — so the check cannot run until the packs answering it exist.

**Config and CLI follow both rule subtasks.** AC9's defaults round-trip compares a no-config run
against a config naming every rule's default per-kind disposition and autofix setting, which requires
every rule to exist rather than only the registry that will hold them.

**AC7's false-positive bar and sample size are recorded on `laconic#bb44` before the corpus run
starts.** A bar set after seeing results is not a bar. This makes the acceptance subtask's first act
a comment, not a run.

### The `implInInterface` ruling and what it costs

`implInInterface` ships the specification's narrow definition: a public subject's doc comment naming
an identifier that is declared in the same file and is not exported. The design's wider version —
naming symbols the subject does not expose — fires on any doc comment that references another public
symbol, which is a legitimate and common cross-reference. Decision and rejected alternative are
recorded on `laconic#bb44`.

**Consequence: the pack interface grows an eleventh concern** — file-level declared symbols with
their visibility. Concern 8 describes the subject; it cannot answer whether an arbitrary name is
private in this file. The design enumerated ten and named that enumeration as charter constraint 4's
protection, so this is an amendment to it rather than a reading of it. What licenses the amendment is
the handover's test — "the first pack needs a concern that is not on the list … that is information,
not a defect" — and the design now states eleven.

**What this makes harder, and what would reverse it:** concern 11 is the concern with the least
evidence behind it and the widest per-language spread — Python has no declaration-level visibility at
all beyond the leading-underscore convention, so its answer is a convention rather than a language
fact. If the probe finds that two or more packs cannot answer concern 11 without heuristics, the
cheaper ruling is the design's wider version with the cross-reference false positives left for AC7 to
measure.

## 2. Traps

What exploration found that the design did not know.

**Rust's doc comment is a grammar field.** In `tree-sitter-rust` v0.24.2, `line_comment` and
`block_comment` each carry an optional `doc` field holding a `doc_comment` child, plus optional
`outer` and `inner` fields holding marker nodes. C8 names three mechanisms — distinct node type,
marker text, position. Rust is a fourth: a field on an ordinary comment node. A pack that reads
`///` as marker text would classify `////////` as Doc kind, which costs `banner` its gate tier and
its Delete fix on exactly the input it exists to catch — the rule still reports it at warn tier with
a Rewrite fix, so the cost is a demotion rather than immunity. Reading the field cannot misclassify
it, because the grammar decides.

**Rust and Java emit no `comment` node, and TS/JS emit two.** Rust and Java produce `line_comment`
and `block_comment`; TypeScript and JavaScript produce `comment` **and** `html_comment`. Concern 2
must rule on `html_comment` explicitly — inheriting it silently puts `<!-- -->` comments in scope for
every rule.

**TS/JS is three grammars behind one pack.** `tree-sitter-typescript` exports two language functions,
`LANGUAGE_TYPESCRIPT` and `LANGUAGE_TSX`; JavaScript is a separate crate. Pack resolution is by file
extension, so a pack selects a grammar **per extension**, while the concern list reads as one grammar
per pack. Build that shape into the trait in the first engine subtask, where Go's single grammar
makes it look redundant. Deferring it to the TS/JS pack means refactoring the trait every pack is
already written against.

**Runtime and grammar versions move independently, and Cargo does not express the coupling.** All six
grammar crates and the `tree-sitter` runtime declare exactly one shared normal dependency,
`tree-sitter-language ^0.1`. A grammar published in 2024 therefore resolves against a 2026 runtime
with no version conflict. C10's pin is per-grammar and correct; what actually has to hold is the
parser ABI, and nothing in the dependency graph will report its violation.

**The age gap is a parser ABI gap, and the runtime spans it.** `tree-sitter-java` 0.23.5 (published
2024-12-21) and `tree-sitter-typescript` 0.23.2 (2024-11-11) report **ABI 14**; Go, Python, Rust and
JavaScript report **ABI 15**. The pinned runtime loads both, so the mixed pin is viable rather than
merely untested — and those two are the pins likeliest to move something when bumped.

**Python's docstring supertype question is settled: there is no hop.** `node-types.json` types
`expression_statement`'s children as the `expression` supertype, but supertypes are hidden in the
tree — the concrete `string` is a direct child, so `(expression_statement (string))` matches with no
traversal. The positional qualifier is the part that needs care: the docstring is the first
**non-extra** child of the scope, and a shebang comment sits before it at module level without
displacing it.

**A subject's body node is not always its statement container.** Go interposes a `statement_list`
between `block` and the statements, so concern 9 counts the named children of `statement_list` for Go
and of `block` or `statement_block` everywhere else. A pack that counts `block`'s children directly
gives Go a statement count of one and makes `density`'s ratio meaningless.

**The design's per-block dispatch count is wrong, and it is wrong by three rules rather than one.**
`density` dispatches per subject. `ignoreReason` and `deadIgnore` are evaluated at reconcile rather
than dispatch, because both take surviving ignore directives as input and `deadIgnore` additionally
needs post-dispatch results. Fifteen rules less those three leaves twelve dispatched per block, where
the design says fourteen. The first subtask corrects the number along with the three other
non-behavioural errors the handover names.

## 3. Verified facts

Checked this session. Nothing here is inherited from the design or the specification.

**Machine, checked by `which` and `clang --version`:** `rustc`, `cargo`, `rustup` and `tree-sitter`
all absent. Apple clang 21.0.0, target `arm64-apple-darwin25.5.0`. Homebrew at
`/opt/homebrew/bin/brew`.

**Crate versions, from `https://crates.io/api/v1/crates/<name>`, `max_stable_version`:**
`tree-sitter` 0.26.12, `tree-sitter-go` 0.25.0, `tree-sitter-python` 0.25.0, `tree-sitter-rust`
0.24.2, `tree-sitter-java` 0.23.5, `tree-sitter-typescript` 0.23.2, `tree-sitter-javascript` 0.25.0,
`tree-sitter-language` 0.1.7.

**Dependencies, from `https://crates.io/api/v1/crates/<name>/<version>/dependencies`, normal kind
only:** every one of the six grammar crates declares `tree-sitter-language ^0.1` and nothing else.
`tree-sitter` 0.26.12 declares `regex ^1.11.3`, `regex-syntax ^0.8.6`, `streaming-iterator ^0.1.9`,
`tree-sitter-language ^0.1`, and `wasmtime-c-api-impl ^36.0.12` as optional.

**Comment-bearing node types, from each grammar's `src/node-types.json` at its version tag on
GitHub:**

| Grammar | Version tag | Node types |
|---|---|---|
| Go | `v0.25.0` | `comment` |
| Python | `v0.25.0` | `comment`; docstrings are `string` under `expression_statement` |
| Rust | `v0.24.2` | `line_comment`, `block_comment`, `doc_comment`, `inner_doc_comment_marker`, `outer_doc_comment_marker` |
| Java | `v0.23.5` | `line_comment`, `block_comment` |
| TypeScript | `v0.23.2` | `comment`, `html_comment` |
| JavaScript | `v0.25.0` | `comment`, `html_comment` |

**Rust comment node fields**, same source, `tree-sitter-rust/v0.24.2/src/node-types.json`: both
`line_comment` and `block_comment` declare `extra: true` and three optional fields — `doc` →
`doc_comment`, `inner` → `inner_doc_comment_marker`, `outer` → `outer_doc_comment_marker`.

**TypeScript language exports**: `LANGUAGE_TYPESCRIPT` and `LANGUAGE_TSX`, both `LanguageFn`
constants. Read from `bindings/rust/lib.rs` on `tree-sitter-typescript`'s **master branch**, not at
the v0.23.2 tag — the probe confirms it at the pin.

**Repository**, from `git log` and `ls .git/hooks`: HEAD `8cb1519` on `main`, tree clean, no remote.
`.git/hooks/` contains `post-commit` and `post-rewrite`, neither under version control. No
`.roborev.toml` exists, so roborev runs on `~/.roborev/config.toml` alone.

## 4. Which existing tests cover this area

**None. The repository contains two markdown files and no code.**

The consequence is that the regression net is a deliverable rather than an inheritance. The fixture
suite AC2 mandates first exists in the subtask that builds the text rules, and every rule and pack
after that extends it; from that point it is the only thing standing between a later subtask and a
silent break in an earlier one. That is why the fixture runner and the `testdata/` exclusion fix
belong to that subtask rather than to whichever subtask first needs a fixture.

Two of the ten acceptance criteria cannot be met before the tool is whole, and the acceptance
subtask owns both: AC1, laconic reporting zero gate findings on its own source with no ignore
directives added, and AC7, the corpus run. Every other criterion is the done-when of the subtask that
builds the thing it tests, which is what keeps the acceptance subtask from becoming the place unmet
criteria accumulate.
