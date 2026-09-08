# The two integrations

laconic's findings are worth nothing unless something runs it without being asked. Both
integrations run the same binary against the same `laconic.toml` and act on the same exit codes:
`0` clean, `1` a gate finding, `2` the run never started.

`--format machine` is the shape to consume from anything that is not a person: one tab-separated
record per finding carrying the rule, the file, the byte span, the line and column, the tier and the
fix shape, followed by an indented `instruction` line. The byte span is there so an agent applies a
Delete without re-deriving the range from the instruction text.

## The pre-commit hook

```sh
cargo build -p laconic-cli
hooks/install.sh
```

`hooks/install.sh` **copies** `hooks/pre-commit` into the repository's hook directory, removing
whatever is there first — `cp` alone writes *through* a symlink, which would leave an older
link-based install in place while reporting success. It installs one file and can be run from
anywhere in the repository.

It refuses to overwrite a hook that is not laconic's, and decides that by the `# laconic-hook:`
marker rather than by comparing content: editing `hooks/pre-commit` is precisely what makes the
content differ, so a content check refused the re-install that the edit calls for.

**A copy rather than a symlink, and re-run it after editing the hook.** The hook directory is shared
by the main checkout and every worktree, while the source file belongs to whichever tree ran the
installer. A link into a worktree dies with `git worktree remove`, and git tests a hook with
`access(X_OK)` — which fails on a dangling link exactly as on a missing file, so every commit
everywhere would silently run no hook. Linking to the main checkout's copy is the other durable
answer and is not available while `hooks/` lives on a feature branch.

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
  Every finding in there is correct and none is actionable. It is 26 of the 69 findings a default
  run reports on `crates/`.
- `target/`, so that `laconic check .` at the repository root means what it looks like it means.

Both are *additions* to the four shipped defaults, which is why the file restates all six. Replacing
rather than extending is required for the opposite case — **removing** a default — and the fixture
suite is what needs it: every fixture lives under `testdata/`, a shipped exclusion, so the suite runs
with the list cleared and would otherwise scan nothing at all.

Regenerate the defaults section with `laconic defaults` after any change to the registry.
