# The two integrations

laconic's findings are worth nothing unless something runs it without being asked. Both
integrations run the same binary against the same `laconic.toml` and act on the same exit codes:
`0` clean, `1` a gate finding, `2` the run never started.

`--format machine` is the shape to consume from anything that is not a person: one tab-separated
record per finding carrying the rule, the file, the byte span, the line and column, the tier and the
fix shape, followed by an indented `instruction` line, and then an indented `note` line where the
finding carries one. The byte span is there so an agent applies a Delete without re-deriving the
range from the instruction text.

**A finding is one, two or three lines, so a parser counts them rather than assuming.** The `note`
is optional — `density` and `docbloat` attach one well past the threshold that bound the subject,
and no other rule attaches any. A consumer that reads exactly two lines per finding takes the note
for the next record, and reports a finding whose rule is the note's first word.

## The pre-commit hook

`hooks/pre-commit` is kept as the reference implementation of the staged-diff check — the logic a
global hook needs, in the smallest form that runs. There is no installer: laconic is moving to a
Claude Code hook installed through werkbank, and a per-repository git hook with its own install path
was a second way to do the same thing, with its own failure modes.

To use it as a git hook anyway, copy it into the repository's hook directory yourself:

```sh
cargo build -p laconic-cli
cp hooks/pre-commit "$(git rev-parse --git-common-dir)/hooks/pre-commit"
chmod +x "$(git rev-parse --git-common-dir)/hooks/pre-commit"
```

`--git-common-dir`, not `--git-dir`: in a worktree the two differ, and hooks live in the common one.
Resolve it from the repository root, since it can be printed as a relative path.

**It does not set `core.hooksPath`.** That setting replaces `.git/hooks` wholesale, so it would
silently disable every hook installed there by anything else — this repository already has a
`post-commit` and a `post-rewrite` hook that are not laconic's.

### Why it lints the staged content and not the file on disk

Every staged file is extracted with `git show :<path>` into a temporary tree at its repository path,
and laconic runs there. The paths are preserved so that exclusions and pack resolution see what a
real run would see, and `--config` is passed explicitly because the temporary tree holds no
`laconic.toml` and discovery from it would walk out of the repository.

Linting the working tree instead would block a commit on edits the author deliberately left
unstaged. A hook that refuses a commit over something the author is not committing is a hook that
gets bypassed, and a bypassed hook catches nothing.

### What it exists to catch

The argument for this hook is measured rather than assumed. In one session, three actors introduced
findings while holding the rules that forbid them:

- a repair agent verified against `docbloat`'s line cap and forgot its per-member ratio, shipping
  two new findings while reporting success;
- a second traded a `docbloat` finding for an `implInInterface` one, having checked the rule it was
  given and not the others;
- the author of this file wrote six of the seven `docbloat` findings on the files that session
  touched, while fixing the rule that reports them, having run laconic on the corpus and on the
  crate after every change and never on the diff.

The shared shape is **verification that depends on remembering to verify**. Putting it in a
checklist did not fix it; the checklist already said to re-run the linter. Linting the diff at the
moment of commit does not depend on anyone remembering.

If no binary is found the hook says so and allows the commit. A missing tool is not a finding, and
failing closed on one teaches people to pass `--no-verify`, which is worse than the gap.

## CI

`.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, and
then laconic over its own source.

Gate findings fail the job; warn findings are printed and do not. That is the tier doing its job
rather than a lenient setting — a warn-tier rule is one the registry says is not yet calibrated
enough to block on. Re-tiering one is a `laconic.toml` edit, which is the point of the file.

## `laconic.toml`

The checked-in file is `laconic defaults` — the shipped defaults rendered from the registry — plus
the two exclusions the engine cannot ship as universal:

- `probe/`, grammar fixtures where a detached comment exists in order to be a detached comment.
  Every finding in there is correct and none is actionable. It is 34 of the 86 findings a default
  run reports on `crates/`.
- `target/`, so that `laconic check .` at the repository root means what it looks like it means.

Both are *additions* to the four shipped defaults, which is why the file restates all six. Replacing
rather than extending is required for the opposite case — **removing** a default — and the fixture
suite is what needs it: every fixture lives under `testdata/`, a shipped exclusion, so the suite runs
with the list cleared and would otherwise scan nothing at all.

Regenerate the defaults section with `laconic defaults` after any change to the registry.
