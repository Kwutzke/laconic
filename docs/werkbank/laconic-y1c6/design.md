# Bash pack and interpreter claims — design

Mode: argumentative. Tracks `laconic#y1c6` under charter `laconic#4wvf`.

This document decides what the bash pack must do, and how a pack comes to claim a file that has no
extension. It does not decide code shape — types, functions, module layout and test names belong to
the plan. Rejected alternatives are not here; they are on `laconic#y1c6`, which outlives this file.

## Contents

- 1. Problem and goal
- 2. Non-goals
- 3. Constraints
- 4. The domain
- 5. Architecture
- 6. Data flow
- 7. Public errors
- 8. Acceptance criteria
- 9. Acceptance journey

## 1. Problem and goal

laconic cannot see a shell script. A `.sh` file resolves no pack, and an extensionless script — the
usual shape of a git hook or a `bin/` tool — is dropped before it is read, because pack resolution
reads only the extension and runs before the file is opened. werkbank, the corpus this work is
judged on, holds 24 shell files; 9 are extensionless with a shebang. laconic's own
`hooks/pre-commit` is one of that shape.

Done is: `laconic check` and `laconic fix` analyse `.sh` and `.bash` files, and any extensionless
file whose first line is a shebang naming `bash` or `sh`, directly (`#!/bin/sh`) or through `env`
(`#!/usr/bin/env bash`); and the bash pack meets every acceptance criterion in the charter.

## 2. Non-goals

1. **zsh, ksh and fish.** tree-sitter-bash emits ERROR nodes on zsh parameter flags such as
   `${(k)assoc}`, and the engine withholds rules on an ERROR subtree. Claiming zsh would claim files
   laconic cannot fully analyse.
2. **Interpreter claims for any other pack.** The interface this work adds lets the Python pack
   claim `python3` without a further engine change; Python opting in is its own issue.
3. **Content sniffing for a path that has an extension.** The extension decides. A `.txt` file
   opening with `#!/bin/sh` stays unclaimed.
4. **Dotfiles without a shebang.** `.bashrc` and `.profile` have no extension by `Path::extension`,
   and rarely a shebang. Claiming them by filename would be a third resolution route.
5. **Shell semantics.** laconic judges comments. What ShellCheck reports is out of scope.

## 3. Constraints

### From the charter

1. **The pack interface gains two concerns, and no engine code names bash** (charter constraint 4).
   The first: a pack declares the interpreter names it claims, and the engine matches them
   generically. The second: a pack judges whether a comment fragment that parsed is code.
   tree-sitter-bash parses a plain sentence such as `The directory we clean up.` as a command with
   no error, so "it parsed" cannot mean "it is code" for shell. Both concerns default to today's
   behaviour, so no existing pack changes, and after this work any pack can use either without a
   further engine change.
2. **Comments come from the parse tree** (charter constraint 2). The shebang is the only text read
   before parsing, it is read only to choose a pack, and it never produces a finding by being read.
3. **Machine-parsed comments are exempt from day one, each with a negative test** (charter
   constraints 5 and 7). That covers the shebang and every `# shellcheck` form — `disable=`,
   `enable=`, `source=`, `shell=`. tree-sitter-bash gives each of them the plain `comment` kind, so
   the exemption is a directive prefix, the mechanism the Python pack already uses for its own
   shebang.
4. **Self-lint stays at zero gate findings** (charter acceptance criterion 1). `hooks/pre-commit`
   becomes laconic source under this work and passes with no ignore directive added.

### From exploration

5. **Interpreter resolution is a fallback.** It runs only for a path with no extension. It reads a
   bounded prefix of the file and tolerates binary and non-UTF-8 content, because the walk visits
   every file — laconic's own `LICENSE` is extensionless — and such a file must cost a short read
   and produce nothing.
6. **`laconic fix` keeps the executable bit.** It holds today because the fixed text is written to
   the same path in place (`crates/laconic-engine/src/run.rs:96`). It becomes a requirement here: a
   hook that `fix` rewrites still runs.
7. **The bash pack supplies its own bound identifiers.** The interface's default collects nodes
   whose kind ends in `identifier`; tree-sitter-bash names functions with `word` and variables with
   `variable_name`, so the default finds nothing and `restate` would be silent.

### Deviations from settled decisions

8. **The extensionless-path contract changes.** Settled: a path with no extension resolves no pack
   and is skipped in silence — stated in `crates/laconic-cli/src/main.rs:203-206`, `README.md:140`,
   `crates/laconic-cli/tests/cli.rs:259-261`, and
   `docs/werkbank/laconic-bb44/design.md:183,349,414`.
   New: such a path is skipped in silence unless its shebang names an interpreter a pack claims.
   Every one of those places is updated to say so.
9. **Charter acceptance criterion 2 is met for bash, not by practice.** The criterion asks for
   positive and negative fixtures per rule under `testdata/<lang>/<rule>/`. Only Go has them; the
   fixture runner names Go alone, and the five other packs ship one carve-out fixture each. The bash
   pack ships per-rule fixtures, so the runner stops being Go-only.

## 4. The domain

1. **Script** — a file the bash pack claims, by one of two routes.
2. **Extension claim** — `.sh` or `.bash`. Consulted first, and final when the path has an
   extension.
3. **Interpreter claim** — for a path with no extension: the first line is a shebang (`#!`), and the
   interpreter it names is one some pack declares. The name is the program's file name
   (`#!/bin/sh` → `sh`), or, when the program is `env`, its first argument that is not a flag
   (`#!/usr/bin/env -S bash -e` → `bash`). The bash pack claims `bash` and `sh`.
4. **Subjects** — the file, and every function definition, in both the `name() { … }` and the
   `function name { … }` forms.
5. **Visibility** — a function whose name begins with `_` is private; every other is exported.
   Sourcing makes everything in a file reachable, so the underscore is the only convention there is.
6. **Doc comment** — bash has no doc syntax, so the pack assigns one, following the Google Shell
   Style Guide, which requires a file header and asks for a header on every function that is not
   both obvious and short:
   - a comment run directly above a function, with no blank line between, documents that function,
     even when it is also the file's first run;
   - otherwise, the first comment run documents the file, provided no code precedes it. The
     shebang and machine directives in that run are not part of it.

   Every other comment is a line comment. The assignment decides which rules apply: `docbloat` and
   `implInInterface` judge only doc comments; `restate`, `commentedOutCode`, `attribution`,
   `detached` and `density` never judge one; `narration` and `banner` gate a line comment but warn
   on a doc comment.

## 5. Architecture

1. **Grammars** gains bash: tree-sitter-bash 0.25.1, the official crate, MIT, ABI 15, compiled by
   `cc` from C sources like every other grammar, with a probe fixture. Verified to build against the
   workspace's `tree-sitter =0.26.12`.
2. **The pack interface** gains interpreter claims, the names a pack claims, and a code judgment on
   a parsed comment fragment. A pack that claims no interpreter and keeps the default judgment
   behaves exactly as it does today, so no existing pack changes. The bash judgment counts a
   fragment as code only when its tree carries shell syntax: a quote, an expansion, a pipe, a list
   operator, a redirect, an assignment, or a `-flag` argument.
3. **Resolution** in the engine becomes two steps: the extension claim, then, for a path with no
   extension, the interpreter claim. Reading the shebang and extracting the interpreter name —
   including the `env` form and its flags — belongs to the engine and is the same for every
   language. Packs declare names and nothing else.
4. **The bash pack** owns what is bash: the comment kind, the directive carve-outs of constraint 3,
   the doc-comment assignment of domain 6, function subjects, underscore visibility, bound
   identifiers (constraint 7), the code judgment `commentedOutCode` consults, and the blank-line
   policy `fix` applies.
5. **The test harnesses** run per-rule fixtures for any language, and resolve files the way the
   engine does, so extensionless test data is exercised by the same route production takes.

## 6. Data flow

1. A path arrives from a directory walk, a named argument, or `--since`. All three reach resolution,
   so `laconic check hooks/pre-commit` analyses the hook.
2. A path with an extension resolves by its extension claim or not at all. An unclaimed extension is
   skipped in silence, as today.
3. A path with no extension has a bounded prefix read. A shebang naming a claimed interpreter
   selects that pack; anything else — no shebang, another interpreter, binary content — is skipped
   in silence.
4. From there the file takes the path every file takes: parse, analyse, dispatch, fix.

## 7. Public errors

1. **No new failure reaches the caller.** An extensionless file that cannot be read during the
   shebang check was never claimed, so it is skipped in silence like an unclaimed extension. A
   claimed file that then fails to read is reported unreadable, as today.
2. **Two packs claiming one interpreter, or one extension, is a defect of the shipped pack set**,
   caught by the test suite before release and never seen at run time. Extension claims carry the
   same invariant, which today they lack: the first registered pack wins in silence.

## 8. Acceptance criteria

Each criterion names the goal (§1), non-goal (§2), constraint (§3) or domain entry (§4) it traces
to.

1. `.sh` and `.bash` files are analysed by the bash pack under `check` and `fix`. *(§1)*
2. An extensionless file opening `#!/bin/sh`, `#!/bin/bash`, `#!/usr/bin/env bash` or
   `#!/usr/bin/env -S bash -e` is analysed by the bash pack, whether reached by a walk, a named
   argument or `--since`. *(§1)*
3. An extensionless file with no shebang, a shebang naming another interpreter (`python3`, `zsh`),
   binary content, or no read permission produces no output and leaves the exit status unchanged. A
   `.txt` file opening with a sh shebang is not analysed. *(§2.1, §2.3, §3.5)*
4. The shebang and every `# shellcheck` form produce no finding, and removing any one carve-out from
   the pack makes the bash carve-out fixture report a finding. *(§3.3)*
5. A `#` inside a heredoc, a quoted string, `${x#prefix}` or `$#` never produces a finding. *(§3.2)*
6. A comment run directly above a function, and a first comment run that no code precedes, are
   judged as doc comments; every other comment as a line comment. *(§4.6)*
7. `commentedOutCode` reports `# rm -rf "$dir"` and `# rm -rf build`, and does not report a prose
   comment such as `# Build the image, then push it`. *(§3.1)*
8. `restate` reports a comment whose words are only those of the bash function or variable it is
   attached to. *(§3.7)*
9. `laconic fix` on a bash file introduces no parse error, handles the blank lines around a removed
   comment correctly, and leaves an executable file executable. *(§3.6; charter acceptance
   criterion 4)*
10. Every rule has positive and negative bash fixtures under `testdata/bash/<rule>/`. *(§3.9)*
11. laconic, `hooks/pre-commit` included, reports zero gate findings on itself with no ignore
    directive added. *(§3.4)*
12. A corpus run covers werkbank and personal-assistant's `bin/hz`: each gate rule's false-positive
    bar is recorded before the run and cleared, and each warn rule's hit rate is recorded. *(charter
    acceptance criterion 3)*
13. Packs that claim no interpreter and keep the default code judgment behave as before, and no two
    shipped packs claim the same interpreter or the same extension. *(§3.1, §7.2)*
14. The README states which interpreters each pack claims and that an extensionless path resolves by
    its shebang. *(§3.8)*

## 9. Acceptance journey

An agent session in werkbank edits `hooks/git-commit-gate`, an extensionless bash script, adding
`# Changed to use jq instead of sed` above a statement. It runs `laconic check --since origin/main`:
the run reports a gate `narration` finding on that line and exits 1. The agent runs `laconic fix`.
The comment is gone, the hook is still executable and still parses, and a second `check` exits 0.
