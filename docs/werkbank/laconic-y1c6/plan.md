# Bash pack and interpreter claims — plan

Mode: argumentative. Tracks `laconic#y1c6`; the design is `design.md` beside this file. The work
items are kata subtasks of `laconic#y1c6`, not listed here. Line numbers are against `origin/main`
at `6d7a8c4`.

## Contents

- Approach and sequencing
- Traps
- Verified facts
- Existing tests at risk
- Verification (end to end)

## Approach and sequencing

0. **Amend the design first.** Constraint 3.1 becomes
   "the pack interface gains two concerns": interpreter claims, and fragment-is-code judgment.
   Domain 4.6 is refined: the first comment run counts as the file's doc only when no code precedes
   it, and a run directly above a function documents the function even when it is also the first
   run. Two rejected alternatives go on the issue: an engine-wide stricter `commentedOutCode`
   (it moves a calibrated gate for six packs), and `commentedOutCode` off by default for bash (it
   needs per-pack rule defaults, which do not exist, and gives up the rule where commented-out
   commands are common).
1. **Grammar.** Pin tree-sitter-bash and add a probe fixture. This is independent and goes first,
   because everything after it needs `Grammar::Bash`.
2. **Interpreter claims and two-step resolution** (engine). The pack trait gains the concern, with a
   default of none. The engine gains shebang extraction and the extension-then-interpreter
   resolution. The contract comments in `main.rs:203-206` and `cli.rs:259-261` change in the same
   commit as the behaviour. Tested with a test-only pack, so it does not wait for the bash pack.
3. **Fragment-is-code concern** (engine). The default keeps today's answer ("it parsed, so it is
   code"), so no existing pack's output changes. Tested with a test-only pack.
4. **The bash pack.** It needs 1, 2 and 3: without 3, the carve-out fixture's zero-findings check
   fails on prose. This covers registration and every per-language table the tests hold.
5. **Generalise the fixture runner and write the bash per-rule fixtures.** The runner change is
   mechanical. The fixtures are 15 rules × positive and negative, and they need the pack.
6. **Self-lint and documentation.** `laconic check .` has to stay clean now that `hooks/pre-commit`
   is analysed. Update the README, and bb44 `design.md:183,349,414`.
7. **Corpus run.** Record the false-positive bars on the issue before running. Then run over
   werkbank and `~/development/personal-assistant/bin/hz` and hand-judge a sample per rule.

Steps 2 and 3 are separate because a reviewer could accept the resolution change and reject the code
judgment.

## Traps

- **`commentedOutCode` reads bash prose as code.** Verified with the probe:
  `The directory we clean up.` and `Build the image, then push it` parse with no error, as a
  `command` with ≥ 3 named nodes. `structural.rs:166-202` runs the bare parse unconditionally and
  `MIN_CODE_NODES = 3` (`structural.rs:24`). Prose with an apostrophe parses to ERROR, and so does
  prose with an unmatched `(`. Step 3 exists because of this. The bash judgment: code only when the
  tree carries shell syntax — a quote, an expansion, a pipe, a list operator, a redirect, an
  assignment, or a `-flag` argument. `rm -rf build` must count as code.
- **`preceding_comment` cannot see `#` directives** (`common.rs:195-217`). It strips with
  `strip_c_markers` (line 208), so `# shellcheck disable=…` directly above a function keeps its
  `#`, matches no prefix, and is returned as the function's doc node. The bash pack needs a
  `#`-aware walk, or the helper takes the pack's `comment_body`.
- **Engine callers of `resolve` have no content; test callers do.** `run.rs:87,103` resolve before
  reading (`:89,111`). The other ~40 call sites in tests already hold the source. Keep the
  extension step path-only, and add the content step for extensionless paths. `check_file`/
  `fix_file` read a bounded prefix only for an extensionless path, and a read failure there is
  silent (design §7.1). A claimed file that later fails to read still goes to `report.unreadable`
  (`run.rs:113-118`).
- **`WithoutCarveOut` does not forward defaulted methods** (`carveouts.rs:27-84`: no
  `statement_scaffold`, no `bound_identifiers`). Both new concerns are defaulted, so they must be
  forwarded. Otherwise the carve-out test runs bash with "parsed means code" and fails for the wrong
  reason.
- **The shebang and the header form one run when adjacent.** Directives are dropped inside a run
  (`pipeline.rs:157-172`), but the doc lookup checks every node in the run, the dropped ones
  included (`pipeline.rs:189-191`). The block's span starts at the first surviving comment
  (`:236`). Returning any one node of the header run as the doc node makes the whole run Doc.
- **The file doc needs the root in `subject_nodes`.** `subject_index` only finds listed nodes
  (`pipeline.rs:136`). Python is the precedent (`python.rs:139-144,159-163,176-177`). A header
  followed by a blank line resolves Detached, but Doc blocks skip `detached` (`registry.rs:174-182`,
  `dispatch.rs:69-71`).
- **A licence header becomes Doc, so the licence carve-out does not remove it** (`pipeline.rs:261`
  requires `kind != Doc`). `docbloat` judges it at warn instead. That is acceptable, and the bash
  carve-out fixture should show it.
- **Declarations table size.** `program` allows 18 child kinds; `compound_statement` allows 31,
  because of its `(( ))` arithmetic form. `function_definition` can sit under if/else/elif, case
  items, do-groups, subshells, lists, pipelines and redirected statements.
  `every_child_of_a_container_is_classified_exactly_once` (`grammar_conformance.rs:174-255`)
  requires every allowed child to be classified. Excluding `if_statement` under `program` makes a
  function inside a top-level `if` a non-declaration (`common.rs:133-149`). Decide that
  deliberately: `if ! command -v x; then x() {…}; fi` is a real idiom.
- **`##` comments.** Python-style `comment_body` strips one `#` (`python.rs:165-168`), so `## Title`
  reaches `banner` with a leading `#`.
- **The test corpus skips extensionless files.** `fix.rs:30` keeps only files with an extension, so
  extensionless bash testdata is never fix-tested unless that filter changes. `fixtures.rs:94-98`
  filters on `go`.
- **kata in the worktree.** The repo alias is not registered there; pass `--project laconic` on
  every call.
- **`config_effect.rs:13` `LANGUAGES` already lacks swift.** That gap predates this work. Add bash;
  Swift's absence is a separate finding.

## Verified facts

- `resolve` is path-only and returns `None` on no extension: `pipeline.rs:58-63`. It is re-exported
  at `lib.rs:29`.
- `Skipped::NoPackClaimsExtension` is defined but never constructed: `pipeline.rs:48-55`.
- `walk` pushes every non-dot, non-excluded file: `run.rs:148-166`. `since::changed_files` has no
  extension filter: `since.rs:52-61`.
- `fix` writes in place, which preserves the mode: `run.rs:96`.
- The grammar macro row format is `Variant => "name", LANGUAGE, NODE_TYPES, "fixture";`
  (`crates/laconic-grammars/src/lib.rs:16,62-74`). The variant count is asserted `== 8` at `:117`.
- Exhaustive per-grammar matches in `crates/laconic-grammars/tests/probe.rs`: `comment_kinds`
  24-31, `expected_abi` 200-205, `expected_kinds` 261-312, `statement_case` 668-679. The truth table
  is 108-126, with its length asserted at 128.
- tree-sitter-bash 0.25.1: MIT, ABI 15, exports `LANGUAGE` and `NODE_TYPES`. The one comment kind is
  `comment`. Built against `tree-sitter =0.26.12` in the scratchpad probe.
- Python's shebang carve-out is the `"!"` prefix: `python.rs:27-30`. Prefix match after
  `trim_start`: `pipeline.rs:314-319`.
- `restate` reads `bound_identifiers` of the following or preceding statement, not only of subjects:
  `pipeline.rs:224-234`. The default collects only `*identifier` kinds: `pack.rs:129-147`. Bash
  uses `word` and `variable_name`.
- Per-language lists: `crates/laconic-packs/src/lib.rs:3-11,17-25`;
  `grammar_conformance.rs:9,17-32,127-136`; `carveouts.rs:107-131` (fixture), `191-231` (tests),
  `237` (generated-marker list), `241` (`#` vs `//`); `pack_concerns.rs:53-68`;
  `config_effect.rs:13`; `crates/laconic-engine/tests/config.rs:11`.
- `fixtures.rs` hardcodes Go at `:94,98,193,215`. `RULES` is at `:128-144`. The sidecar is
  `line:rule` (`:82-91`). `density` is enabled only in its own directory (`:56-67`).
- CI self-lint is `cargo run --release -p laconic-cli -- check .`
  (`.github/workflows/ci.yml:32-33`). The tracked extensionless files are `LICENSE` and
  `hooks/pre-commit`. There are no `*.sh` files.
- `hooks/pre-commit` opens with a 10-line header. As a file doc it exceeds `absolute_doc_lines = 6`
  (`laconic.toml`), so `docbloat` warns. Warn does not fail CI.

## Existing tests at risk

- `go_pipeline.rs:128-136` `pack_resolution_is_by_extension`: `Makefile` resolves `None`. It must
  stay so; the path does not exist and has no shebang.
- `cli.rs:163-175` `an_unclaimed_extension_is_silent`, `cli.rs:177-201` unreadable-file reporting,
  `cli.rs:257-275` mistyped short flag (its doc states the old contract).
- `pack_concerns.rs:48-80` `every_claimed_extension_resolves`.
- `carveouts.rs`: every test, through `WithoutCarveOut` and the marker choice.
- `fix.rs` AC5/AC6 corpus loops (`:62-115`).
- `fixtures.rs`: the whole suite, through the runner change.
- `crates/laconic-grammars/tests/probe.rs`: variant count, truth table, fixture parse and
  comment-kind tests.
- `grammar_conformance.rs`: `lists()` kind existence; the container classification test.
- `crates/laconic-engine/tests/config.rs` `LANGUAGES` validation.

## Verification (end to end)

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace` all pass.
- `cargo run --release -p laconic-cli -- check .` exits 0.
- Acceptance journey, in a scratch copy of werkbank: add `# Changed to use jq instead of sed` to
  `hooks/git-commit-gate`. `check --since origin/main` exits 1 with `narration`. `fix` removes it,
  `test -x hooks/git-commit-gate` passes, and a second `check` exits 0.
