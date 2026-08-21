# laconic v1 — design

Mode: argumentative. Tracks `laconic#bb44` under charter `laconic#4wvf`.

This document decides what laconic must be and do: the domain it models, the units it is built from,
the boundary between the engine and a language pack, the rule set with its tiers and fix shapes, and
what has to be observably true before v1 closes. It does not decide code shape — types, functions,
module layout, query strings and test names belong to the plan. Rejected alternatives are not here;
they are on `laconic#bb44`, which outlives this file.

## Contents

- 1. Problem and goal
- 2. Non-goals
- 3. Constraints
- 4. The domain
- 5. Architecture — the four units, the doc-comment deletion invariant, the fifteen rules, the
  carve-outs
- 6. Data flow
- 7. Public errors
- 8. Acceptance criteria

## 1. Problem and goal

Comments that carry no information survive review because deleting them is nobody's job and each one
is individually too small to argue about. They accumulate fastest in code an LLM wrote, because the
generating model narrates its own edit (`// changed to use a map for O(1) lookup`), restates the line
below it, and leaves banners behind. The cost is not the bytes. It is that a reader learns to skip
comments, and then skips the three that mattered.

laconic finds those comments and deletes them, in any language it has a pack for, with no model in
the loop. A single static binary that runs standalone, as a pre-commit hook, and in CI.

The consumer that shapes every other decision in this document is an LLM acting on one diagnostic
with no surrounding context. It will read `remove this comment` and remove the comment. It will read
`narration detected` and write a comment explaining that the narration was detected. Diagnostics are
therefore instructions naming the change, never labels naming the defect.

The second consequence of that consumer is defensive: the agent is optimising for a green run, and
will take any deletion that reaches green. Every rule in section 5 is designed against that, not
against a cooperative human.

## 2. Non-goals

**Semantic judgment.** Whether a comment is misleading, and whether non-obvious code is missing a
comment it needs, are not statically decidable. They belong to review. No amount of rule tuning moves
them into scope, and a rule that appears to decide one is a rule that is wrong on inputs nobody
tested.

**Being a syntax checker.** A file that does not parse is the compiler's problem. laconic reports what
it can still see and never exits non-zero for a parse failure alone (section 7).

**Style and formatting.** Comment wrapping, capitalisation, terminal punctuation, and the position of
`//` relative to the code are a formatter's job.

**Narration inside doc comments is out of scope for deletion, not for detection.** Section 5 states
the invariant and its consequence.

## 3. Constraints

The charter `laconic#4wvf` carries constraints 1–7. Two more come from what exploration established:

**C8 — no language identifies a doc comment by node type, so a pack cannot declare one.** Two
identify it by position and three by text marker. In tree-sitter-python there is no docstring node: a
docstring is `(expression_statement (string))`, and that pattern only means docstring when it is the
first statement in a module, class, or function body. A string expression anywhere else is a string.
Verified this session against [tree-sitter discussion
#2470](https://github.com/tree-sitter/tree-sitter/discussions/2470). Go's is likewise positional — a
comment directly above a top-level declaration with no blank line. Rust, TypeScript and Java identify
theirs by marker text on an ordinary comment node, which is text inspection rather than a node type.
The pack therefore supplies a predicate that returns a doc comment *and the subject it documents*,
because three rules need facts about the subject and not just the comment.

**C9 — no Rust toolchain exists on the build machine.** `rustup`, `cargo` and the `tree-sitter` CLI
are all absent; Apple clang 21 is present, so grammar C sources compile once the toolchain is
installed. Verified this session. Toolchain install is the first step of implementation, not a risk.

## 4. The domain

**Comment block.** The unit every rule operates on. Consecutive comment nodes separated by whitespace
only, each alone on its line, form one block. A comment with code before it on the same line is
always its own block and never merges. A four-line narration block is one finding carrying one fix
spanning four lines — not four findings an autofixer applies in four passes.

**Attachment.** What a block relates to, in one of three states. *AttachedBelow*: the next non-comment
node follows with no blank line between. *AttachedTrailing*: code precedes the comment on its line.
*Detached*: a blank line intervenes, or nothing follows. The `detached` rule is exactly
`attachment == Detached`, which is why the Go build-constraint carve-out has to live in the pack: a
`//go:build` line is required to be followed by a blank line, so it is structurally detached and
correct.

**Kind.** *Line*, *Block*, or *Doc*. The pack assigns it. Every rule declares a disposition per kind,
and this is the whole mechanism by which language asymmetries stay out of the rules: the Python pack
assigns Doc to a first-statement string and Line to a `#` comment, and `narration` declaring its
dispositions then handles Python correctly without containing any Python.

**Subject.** What a block is attached to, described by the pack rather than handed over as a raw
syntax node. A subject reports the identifiers it binds (`restate`), the length of its body
(`docbloat`, `density`), and its visibility together with the symbols it exposes (`implInInterface`).
Handing a rule a raw node puts a per-language match arm inside three rules and ends the pack
abstraction.

**Directive.** A comment the machine reads — `//go:build`, `# type:`, `@ts-expect-error`,
`// CHECKSTYLE:OFF`. Pack-declared, and removed before any rule sees the block. A linter that fires on
a directive is a linter that gets disabled on day one.

**Excluded region.** File-level or span-level, and applied before rules: license and copyright headers
at top of file, generated-file markers, and any path under `testdata/`, `fixtures/`, `vendor/`, or
`node_modules/`.

**Finding.** A rule id, a byte span, a severity, an instruction, and a fix shape. **Fix shape** is
*Delete*, *Rewrite*, or *None*. Only *Delete* is mechanically applicable.

## 5. Architecture

Four units.

**Engine.** Owns the pipeline and nothing about any language. Resolves a pack by file extension,
parses, extracts comment nodes, groups them into blocks, resolves attachment and kind, builds
subjects, drops excluded regions and directives, consumes ignore directives, dispatches surviving
blocks to enabled rules, and collects findings.

**Registry.** The rule set as declarations. Each entry carries an id, a per-kind disposition, a
default severity, and a default autofix setting. Enabling, disabling, re-tiering and configuring a
rule all act on this entry, so `ignoreReason` and `deadIgnore` are ordinary entries rather than
engine behaviour with no configuration surface.

**Packs.** All per-language knowledge. A pack is a data value implementing the pack interface, built
from declarations plus one named strategy per positional concern. There are two such concerns, and
across the supported languages each collapses onto a small strategy set:

| | Go | Python | Rust | TS/JS | Java |
|---|---|---|---|---|---|
| Doc comment | preceding decl | first-statement string | marker `///` `//!` | marker `/**` | marker `/**` |
| Visibility | first rune case | leading underscore | `pub` modifier | `export` keyword | `public` modifier |

Three doc strategies and four visibility strategies cover every language in v1, which is the evidence
that a pack is mostly a table. The interface stays open for a language whose scheme fits no existing
strategy, so charter constraint 4 does not have to be broken to add the sixth language.

*Bounded claim:* the Python doc-comment strategy is verified (C8). The visibility row states language
semantics, which are not in doubt. What is unverified is how each strategy maps onto its grammar's
node structure — every cell but the Python doc-comment one is design intent rather than a checked
parse. Each mapping is confirmed against a real parse tree in the first implementation subtask; a
mismatch changes which strategies exist, not the pack interface.

**Fix.** Turns *Delete* findings from autofix-enabled rules into byte-range edits and applies them
back-to-front so earlier offsets stay valid. Owns the blank-line policy around a removed block, which
is per-language and is the part that makes fix mode a unit rather than a loop.

### The doc-comment deletion invariant

**No rule with a Delete fix shape applies to Doc kind.**

A doc comment is a public surface. Go's feeds pkg.go.dev, Rust's feeds rustdoc, Java's and
TypeScript's feed their generators — and Python's is a runtime value on `__doc__`, so deleting one is
a behavioural change that can break `help()` and doctests. Deleting a doc comment is an API change
wearing a cleanup's clothes, and the agent from section 1 cannot tell the difference.

The consequence is not that narration in a doc comment goes undetected. It is that the same rule
carries a different disposition there: Delete at gate tier for Line and Block kinds, Rewrite at warn
tier for Doc. `// Updated to return an error instead of panicking` above an exported Go function is
exactly the slop laconic exists for, and the correct fix is to rewrite the doc comment to describe
present behaviour — never to delete the documentation.

### The rule set

Fifteen rules. Thirteen from the specification, plus two the specification's own text requires but
does not list.

**Gate tier, Delete fix, autofix on by default.** Exit non-zero; `laconic fix` removes them.

| Rule | Kinds | Fires on |
|---|---|---|
| `narration` | Line, Block — plus Doc at warn tier with a Rewrite fix | change-log and process language: *changed to*, *updated to*, *previously*, *as requested*, *now we*, *note that we*, *moved to*, *refactored to*, *this now*, *per your* |
| `banner` | Line, Block — plus Doc at warn tier with a Rewrite fix | `^\s*[-=*_#]{3,}$`, `Step N:` sequences, standalone all-caps section labels |
| `detached` | Line, Block | `attachment == Detached` |

**Gate tier, Delete fix, autofix off by default.** Exit non-zero; `laconic fix` leaves them for a
human until the corpus run (section 8) justifies flipping the default.

| Rule | Kinds | Fires on | Why autofix is off |
|---|---|---|---|
| `restate` | Line, Block | content tokens are a subset of the identifiers the subject binds, split on camelCase and snake_case, after stopword removal | A comment naming the same identifiers while saying *why* is a subset by this test and is the most valuable comment in the file |
| `commentedOutCode` | Line, Block | body parses in the file's language with zero ERROR nodes, above a minimum token count | Prose parses as valid code more often than expected — `// handle user input` is a call expression in several grammars |
| `attribution` | Line, Block | *written by*, *authored by*, *adapted from*, *courtesy of*, *based on code from* | *adapted from* can be a required licence notice. The header carve-out catches the top-of-file case; a mid-file notice is indistinguishable from vanity by any deterministic test, and deleting the wrong one is a licensing problem |

**Gate tier, no fix.** Exit non-zero; a human decides.

| Rule | Kinds | Fires on |
|---|---|---|
| `fileref` | all | a source-file path per the pack's extension list. The instruction names the alternative: *reference a symbol, not a file* |
| `ignoreReason` | all | `laconic:ignore <rule>` with no reason. **Never autofixable**: the only deletion that makes it pass is deleting the directive, which silently re-enables the rule it suppressed — and that is the first fix a green-seeking agent finds |

**Warn tier, Rewrite fix, never autofixable.** Reported, do not affect exit status.

| Rule | Kinds | Fires on |
|---|---|---|
| `density` | Line, Block | per subject: more than 8 comment lines **and** comment-to-statement ratio above 0.5 |
| `docbloat` | Doc | over 15 lines, or over 3× the subject's body |
| `implInInterface` | Doc | a public doc comment naming symbols its subject does not expose |
| `hedging` | all | *should work*, *for now*, *in most cases*, *if needed*, *probably*, *might need* |
| `vague` | all | *handles the logic*, *does the necessary*, *various things*, *as appropriate*, *etc.* as a sentence ender |
| `task` | all | TODO/FIXME/XXX/HACK with no issue reference. `TODO(KAT-123)` and `TODO(#456)` pass |
| `deadIgnore` | all | a directive whose named rule did not fire on the block it protects |

*Bounded claim:* the thresholds — 8 comment lines, ratio 0.5, 15 doc lines, 3× body — are the
specification's initial values, not measurements. They are configurable, and the corpus run in
section 8 is what turns them into calibrated values.

### Carve-outs

Universal across packs: licence and copyright headers at top of file, generated-file markers, and
anything under `testdata/`, `fixtures/`, `vendor/`, `node_modules/`.

| Language | Must never fire on |
|---|---|
| Go | `//go:*`, `// Code generated by … DO NOT EDIT.`, build-constraint blocks |
| TS/JS | `/// <reference path=…>`, `//# sourceMappingURL=`, `@ts-ignore`, `@ts-expect-error`, `eslint-disable*`, `prettier-ignore`, webpack magic comments |
| Python | `#!` shebang, `# type:`, `# noqa`, `# pragma: no cover`, `# -*- coding:` |
| Rust | `// rustfmt::skip`. `#[…]` attributes are not comments and never reach a rule |
| Java | `// CHECKSTYLE:*`, `// NOPMD`. `@SuppressWarnings` is an annotation, not a comment |

## 6. Data flow

`paths → per file: resolve pack by extension → parse → extract comment nodes → group into blocks →
resolve attachment, kind, subject → drop excluded regions and directives → consume ignore directives
→ dispatch to enabled rules → findings`.

In fix mode, findings whose rule has autofix enabled and a Delete fix shape become byte-range edits,
applied back-to-front within a file.

Files share nothing but the config and the registry, so the per-file pipeline parallelises without
coordination. Ordering of findings across files is imposed at output, not by the walk.

## 7. Public errors

**Exit codes.** `0` — no gate findings. `1` — gate findings present. `2` — usage or configuration
error. CI must be able to distinguish a failing gate from a broken install, which one non-zero code
cannot express.

**A file with ERROR nodes is processed, not skipped.** Comment extraction survives ERROR nodes, so
`narration`, `banner`, `attribution`, `hedging`, `vague` and `task` remain correct. Rules whose
correctness depends on subject resolution — `restate`, `detached`, `docbloat`, `implInInterface`,
`density` — are suppressed within any subtree containing an ERROR node, and the suppression is
reported as a note naming the file and the rules withheld. A parse failure never by itself produces a
non-zero exit.

The note is the caller's business rather than a log line, because the alternative is that a file
which cannot be fully analysed is indistinguishable from a clean one.

**A reasonless ignore directive is a finding, not a fatal.** It surfaces through `ignoreReason` at
gate tier, so a run reports every one of them rather than aborting on the first.

**An unreadable file and an unparseable config differ.** An unreadable or non-UTF-8 file is reported
per file and the run continues over the rest. A malformed `laconic.toml`, an unknown rule id in it,
or an unknown language override aborts before any file is read, with exit 2 — a config the tool
half-understands would silently disable rules the author believed were on.

**An extension with no pack is not an error.** The file is skipped silently. laconic runs over whole
repositories, and warning on every `.json` and `.md` makes the output unusable.

## 8. Acceptance criteria

1. `laconic` reports zero gate-tier findings on its own source, with no ignore directives added to
   reach that state. A rule that fires on reasonable code in this repository is a wrong rule and is
   changed, not suppressed.
2. Every rule has positive and negative fixtures under `testdata/<lang>/<rule>/`, carrying expected
   diagnostic annotations.
3. Every carve-out named in section 5 has an explicit negative test in its pack, and each test fails
   when its carve-out is removed.
4. Deny-list matching is tested for case and word boundaries: `fixed` does not match `fix`, and
   `previously` does not match inside `previouslyKnownAs`.
5. `laconic fix` output parses in the source language with zero ERROR nodes, and blank-line handling
   around each removed block matches the language's convention.
6. Running `laconic fix` twice produces the same file as running it once.
7. A corpus run over one real repository per supported language, with a hand-judged sample of findings
   per rule. The false-positive bar and the sample size are recorded on `laconic#bb44` before the run
   begins — a bar set after seeing the results is not a bar. Each gate-tier rule clears it; each
   warn-tier rule has its hit rate recorded. Where a rule misses the bar, the recorded outcome is a
   threshold or deny-list change, or its autofix default staying off — never a suppression in the
   corpus.
8. Every diagnostic message states the change to make. A message that names a defect without naming
   the change fails this criterion regardless of how accurate it is.
9. `laconic.toml` documents every rule's default tier, severity and autofix setting, and a run with no
   config file behaves identically to a run with a config file containing only those defaults.
