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
- 5. Architecture — the five units, the pack interface, the doc-comment deletion invariant, the
  fifteen rules, the carve-outs
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

**C8 — how a doc comment is identified varies by language across three unrelated mechanisms, so a
pack cannot declare a single one.** Some grammars expose a distinct node type. Some require reading
marker text off an ordinary comment node. Some settle it only by position, with no lexical signal at
all: in tree-sitter-python a docstring is `(expression_statement (string))`, and that pattern means
docstring only when it is the first statement in a module, class, or function body — a string
expression anywhere else is a string ([tree-sitter discussion
#2470](https://github.com/tree-sitter/tree-sitter/discussions/2470) establishes the node pattern; the
first-statement rule is Python's own semantics, not that thread's). Go is positional in the same way,
by a comment sitting directly above a top-level declaration with no blank line.

A pack therefore supplies a **predicate**, not a declaration, and that predicate returns the doc
comment *together with the subject it documents* — because three rules need facts about the subject
and not just the comment. Which mechanism each language uses is a value the pack carries, not a
premise this design rests on.

**C9 — no Rust toolchain exists on the build machine.** `rustup`, `cargo` and the `tree-sitter` CLI
are all absent; Apple clang 21 is present, so grammar C sources compile once the toolchain is
installed. Verified this session. Toolchain install is the first step of implementation, not a risk.

**C10 — grammar node shapes move between releases, so every grammar is version-pinned and every
mapping is re-verified when a pin moves.** Node types are renamed, split and hidden across grammar
versions: a pattern that matches today can match nothing after a minor release. A pin is not a
precaution here, it is the only thing that makes the pack tables checkable at all.

## 4. The domain

**Comment block.** The unit a rule receives. Consecutive comment nodes separated by whitespace only,
each alone on its line, form one block. A comment with code before it on the same line is always its
own block and never merges. A four-line narration block is one finding carrying one fix spanning four
lines — not four findings an autofixer applies in four passes.

The cost of that is real and is accepted: **deletion is all-or-nothing**. A four-line block whose
third line is the one sentence worth keeping is deleted whole, and no fix shape can express partial
deletion. Per-comment granularity would avoid it at the price of four rules reimplementing grouping
and `fix` merging overlapping adjacent deletions, which is the worse trade.

One rule does not fit this unit. `density` is an aggregate over every block attached to one subject,
so it is dispatched **per subject** and its finding anchors to the subject rather than to a block.
That is the only per-subject dispatch in the set.

**Attachment.** What a block relates to, in one of three states. *AttachedBelow*: the next non-comment
node follows with no blank line between. *AttachedTrailing*: code precedes the comment on its line.
*Detached*: a blank line intervenes, or nothing follows. The `detached` rule is exactly
`attachment == Detached`.

**Kind.** *Line*, *Block*, or *Doc*. The pack assigns it. Every rule declares a disposition per kind,
and this is the whole mechanism by which language asymmetries stay out of the rules: the Python pack
assigns Doc to a first-statement string and Line to a `#` comment, and `narration` declaring its
dispositions then handles Python correctly without containing any Python.

**Subject.** What a block is attached to, described by the pack rather than handed over as a raw
syntax node. A subject reports the identifiers it binds (`restate`), the length of its body
(`docbloat`, `density`), and its visibility together with the symbols it exposes (`implInInterface`).
Handing a rule a raw node puts a per-language match arm inside three rules and ends the pack
abstraction.

**Machine directive.** A comment some other tool reads — `//go:build`, `# type:`, `@ts-expect-error`,
`// CHECKSTYLE:OFF`. Pack-declared, and removed before any rule sees the block. A linter that fires on
one of these is a linter that gets disabled on day one. This is also what protects Go build
constraints: they are stripped as directives, so no rule reaches them and no rule needs a special
case for them.

**Ignore directive.** laconic's own suppression, `laconic:ignore <rule> — <reason>`. It is **not** a
machine directive and is never stripped, because two rules take it as their input: `ignoreReason`
fires on one lacking a reason, and `deadIgnore` fires on one whose named rule ran and did not fire. It
is therefore retained as data and evaluated after dispatch.

Three things about it are the caller's contract and are fixed here:

- **It binds to the block below it, or to the block on its own line when trailing.** An ignore
  directive on its own line does not merge into the block it protects — it is lifted out during
  grouping. Otherwise its own text would join the block and change what `narration` and `restate`
  match against, so a suppression would alter the finding it suppresses.
- **A protected block is dispatched, not skipped.** Its rules run and their findings are recorded as
  suppressed rather than discarded. This is what lets `deadIgnore` know whether the named rule would
  have fired, without a second evaluation pass.
- **`fix` removes an ignore directive together with the block it protects.** A directive left behind
  after its block is deleted would become a `deadIgnore` finding manufactured by `fix` itself, and
  would break the idempotence AC6 requires.

**Excluded region.** File-level or span-level, and applied before rules: license and copyright headers
at top of file, generated-file markers, and any path under `testdata/`, `fixtures/`, `vendor/`, or
`node_modules/`. **The path set is configurable and replaceable**, which is not a convenience: AC2
puts every fixture under `testdata/<lang>/<rule>/`, so the fixture suite runs with the path
exclusions cleared and would otherwise scan nothing at all.

**Finding.** A rule id, a byte span, a tier, an instruction, and a fix shape. **Fix shape** is
*Delete*, *Rewrite*, or *None*. Only *Delete* is mechanically applicable.

## 5. Architecture

Five units.

**Engine.** Owns the pipeline and nothing about any language. Resolves a pack by file extension,
parses, extracts comment nodes, groups them into blocks, resolves attachment and kind, builds
subjects, drops excluded regions, strips machine directives, lifts out ignore directives, dispatches
to enabled rules, and reconciles suppressions into findings.

**Registry.** The rule set as declarations. Each entry carries an id, a **disposition per comment
kind** — the (tier, fix shape) pair that kind gets, or nothing where the rule does not apply — and a
default autofix setting. Disposition is per-kind rather than per-rule because `narration` is gate plus
Delete for Line and Block and warn plus Rewrite for Doc; a single tier per rule cannot express that.

Charter constraint 6 names two axes, tier and autofix, and the registry carries exactly those. There
is no separate severity: tier decides exit status, and the reporter emits tier directly.

Enabling, disabling, re-tiering and configuring a rule all act on this entry, so `ignoreReason` and
`deadIgnore` are ordinary entries rather than engine behaviour with no configuration surface.

**Packs.** All per-language knowledge, and the interface is exactly these ten concerns. Four are
declarations — lists a pack states. Six are strategies — predicates a pack answers, because no list
can express them.

| # | Concern | Shape | Who needs it |
|---|---|---|---|
| 1 | Source extensions this pack claims | declaration | pack resolution, and `fileref`'s path detection |
| 2 | Which grammar node types carry comments | declaration | extraction. There is no node type common to all grammars, so this cannot be an engine constant |
| 3 | Machine-directive prefixes | declaration | stripping, before any rule runs |
| 4 | Generated-file markers | declaration | file-level exclusion |
| 5 | Comment kind — Line, Block or Doc | strategy | every rule, via its per-kind disposition |
| 6 | The doc comment and the subject it documents | strategy | `docbloat`, `implInInterface` (C8) |
| 7 | Comment body, with markers stripped | strategy | `banner` and every deny-list rule, which match content and must not see `//`, `#`, `/**` or `*` continuation leaders |
| 8 | A subject's visibility and exposed symbols | strategy | `implInInterface`. Not a boolean — `pub(crate)`, a TypeScript class member, and a package-level Go identifier are different questions |
| 9 | A subject's statement count | strategy | `density`'s ratio |
| 10 | Blank-line policy around a removed block | strategy | `fix` |

**What protects charter constraint 4 is this enumeration plus the open trait — not a claim that the
strategies collapse.** An earlier draft argued that three doc strategies and four visibility
strategies cover all five languages, therefore a pack is mostly a table, therefore constraint 4 is
safe. That argument was computed over two of these ten concerns and does not survive the other eight.
The weaker claim is the true one: the interface is knowable and finite, a new language fills ten
slots, and a language whose strategy fits nothing already there implements the trait directly rather
than forcing a new variant into the engine. Constraint 4 holds because the engine has no per-language
branch to add to, not because every language turns out to be similar.

*Bounded claim:* which mechanism each language uses for concerns 5 through 10 is design intent, not
checked parses. Each is confirmed against a real parse tree in the first implementation subtask,
against the pinned grammar version C10 requires. A mismatch changes a pack's table row; it does not
change this interface, which is the property that makes the deferral safe.

**Fix.** Turns *Delete* findings from autofix-enabled rules into byte-range edits and applies them
back-to-front so earlier offsets stay valid. Resolving what whitespace survives a removal is the part
that makes this a unit rather than a loop — but the policy itself is the pack's (concern 10), not
`fix`'s, because a per-language switch inside `fix` is the same constraint-4 violation as one inside
the engine.

**Reporter.** Owns what a caller receives: the findings, in a stable order, together with the
withheld-rules notes described under public errors. This is a unit rather than a formatting detail
because the consumer named in section 1 is a machine acting on one diagnostic, which makes the output
a contract:

- **Two formats, one source.** A human-readable form for a terminal, and a machine-readable form for
  everything else. The machine form carries, per finding: rule id, file, byte span, line and column,
  tier, the instruction, and the fix shape — with a *Delete* fix expressed as a span an agent can
  apply without re-deriving it from prose.
- **Ordering is imposed here**, by file path then byte offset, so two runs over the same tree produce
  identical output and a CI diff means something. The walk's order is not the output's order.
- **The withheld-rules note from §7 is a first-class record, not a log line** — a file laconic could
  not fully analyse must be distinguishable from a clean one in the output itself.

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
| `banner` | Line, Block — plus Doc at warn tier with a Rewrite fix | a stripped body that is only repeated punctuation, a `Step N:` sequence, or a standalone all-caps section label. Matching is over the body from pack concern 7, never the raw line — a raw-line test both misses `// =====` and fires on a Markdown rule |

**Gate tier, Delete fix, autofix off by default.** Exit non-zero; `laconic fix` leaves them for a
human until the corpus run (section 8) justifies flipping the default.

| Rule | Kinds | Fires on | Why autofix is off |
|---|---|---|---|
| `restate` | Line, Block | content tokens are a subset of the identifiers the subject binds, split on camelCase and snake_case, after stopword removal | A comment naming the same identifiers while saying *why* is a subset by this test and is the most valuable comment in the file |
| `commentedOutCode` | Line, Block | body parses in the file's language with zero ERROR nodes, above a minimum token count | "Parses" is weaker than "is code", and by a different amount per grammar: several accept bare statements at top level so that partial snippets parse at all. Short prose clears the bar — a single word is a valid expression statement in most of them, which is what the token minimum exists to stop |
| `attribution` | Line, Block | *written by*, *authored by*, *adapted from*, *courtesy of*, *based on code from* | *adapted from* can be a required licence notice. The header carve-out catches the top-of-file case; a mid-file notice is indistinguishable from vanity by any deterministic test, and deleting the wrong one is a licensing problem |
| `detached` | Line, Block | `attachment == Detached` | *Detached* includes "nothing follows", so `// intentionally empty`, `// no-op` and `// unreachable — checked by the caller` at the end of a block are structurally identical to an orphan. It is the only rule here with no text test at all, which makes it the most destructive one to run unattended |

These four gate without a mechanical remedy, which is a workflow consequence rather than an
oversight: in a pre-commit hook the only way past one is to fix it by hand or to write a reasoned
ignore directive — which `ignoreReason` then polices and `deadIgnore` later flags when it goes stale.
That friction is the point, and it is the price of not deleting good comments unattended.

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
| `deadIgnore` | all | an ignore directive whose named rule **ran and did not fire** on the block it protects. Never fires for a rule that was suppressed rather than evaluated |

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
| Rust | `// SAFETY:`, which clippy's `undocumented_unsafe_blocks` requires — deleting one breaks a lint. rustfmt and clippy are otherwise controlled by `#[…]` attributes, which are not comments and never reach a rule, so Rust's carve-outs are comment-borne conventions rather than tool directives |
| Java | `// CHECKSTYLE:*`, `// NOPMD`. `@SuppressWarnings` is an annotation, not a comment |

## 6. Data flow

`paths → per file: resolve pack by extension → parse → extract comment nodes → group into blocks →
resolve attachment, kind, subject → drop excluded regions → strip machine directives → lift out
ignore directives → dispatch → reconcile → report`.

Two stages carry more than their name. **Dispatch** is per block for fourteen rules and per subject
for `density`, over the blocks attached to that subject. **Reconcile** is the stage an earlier draft
lacked: findings covered by an ignore directive are marked suppressed rather than discarded, and only
then can `deadIgnore` ask whether a named rule ran and did not fire, and `ignoreReason` report a
directive carrying no reason. Both rules read ignore directives that survived dispatch, which is why
those are lifted out rather than stripped — a stripped directive is invisible to the two rules that
take it as input.

In fix mode, findings whose rule has autofix enabled and a Delete fix shape become byte-range edits,
applied back-to-front within a file, each carrying its protecting ignore directive with it.

Files share nothing but the config and the registry, so the per-file pipeline parallelises without
coordination. Output order is the reporter's, not the walk's.

## 7. Public errors

**Exit codes.** `0` — no gate findings. `1` — gate findings present. `2` — usage or configuration
error. CI must be able to distinguish a failing gate from a broken install, which one non-zero code
cannot express.

**A file with ERROR nodes is processed, not skipped**, and the partition below covers every rule in
the set — a rule missing from it is a rule whose behaviour on broken input nobody decided.

*Unaffected*, because their test is over comment text or over the comment body's own nested parse,
neither of which depends on the surrounding tree: `narration`, `banner`, `attribution`, `hedging`,
`vague`, `task`, `commentedOutCode`, `fileref`, `ignoreReason`.

*Suppressed within any subtree containing an ERROR node*, because their correctness depends on
subject resolution and attachment, both unreliable there: `restate`, `detached`, `docbloat`,
`implInInterface`, `density`.

*Conditional*: `deadIgnore` fires only when its named rule **ran and did not fire**, never when the
rule was suppressed. Suppressed is not the same as did-not-fire, and conflating them reports a live
ignore directive as dead — whereupon deleting it, as the diagnostic instructs, makes the suppressed
rule fire the moment the syntax error is fixed.

The suppression is reported as a note naming the file and the rules withheld. A parse failure never
by itself produces a non-zero exit.

The note is the caller's business rather than a log line, because the alternative is that a file
which cannot be fully analysed is indistinguishable from a clean one.

**A reasonless ignore directive is a finding, not a fatal.** It surfaces through `ignoreReason` at
gate tier, so a run reports every one of them rather than aborting on the first.

**An unreadable file and an unparseable config differ.** An unreadable or non-UTF-8 file is reported
per file and the run continues over the rest. A malformed `laconic.toml`, an unknown rule id in it, or
a language override naming a language no pack claims aborts before any file is read, with exit 2 — a
config the tool half-understands would silently disable rules the author believed were on. (**A
language override** is a per-language block that re-tiers, disables, or re-configures a rule for one
language only; it is the mechanism by which a rule that is right for Go and wrong for Python is
settled without removing it from the set.)

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
5. `laconic fix` introduces no parse errors: for any file that parsed with zero ERROR nodes, the fixed
   output parses with zero ERROR nodes. Stated conditionally because §7 requires files that already
   contain ERROR nodes to be processed, and no fix can remove a syntax error it did not create.
   Blank-line handling around each removed block matches the language's convention.
6. Running `laconic fix` twice produces the same file as running it once.
7. A corpus run over one real repository per supported language, with a hand-judged sample of findings
   per rule. The false-positive bar and the sample size are recorded on `laconic#bb44` before the run
   begins — a bar set after seeing the results is not a bar. Each gate-tier rule clears it; each
   warn-tier rule has its hit rate recorded. Where a rule misses the bar, the recorded outcome is a
   threshold or deny-list change, or its autofix default staying off — never a suppression in the
   corpus.
8. Every diagnostic message states the change to make. A message that names a defect without naming
   the change fails this criterion regardless of how accurate it is.
9. `laconic.toml` documents every rule's default per-kind disposition and autofix setting, and a run
   with no config file behaves identically to a run with a config file containing only those
   defaults.
10. Two runs over an unchanged tree produce byte-identical machine-format output, and every finding in
    it carries a span an agent can apply without parsing the instruction text. The human-readable and
    machine-readable forms report the same findings and the same withheld-rule notes.
