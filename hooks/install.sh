#!/bin/sh
# Install hooks/pre-commit into this repository's hook directory.
#
# Two things this deliberately does not do: set core.hooksPath, which replaces .git/hooks wholesale
# and would disable every hook installed there by anything else; and overwrite a pre-commit hook
# that is already present and not ours.
set -eu

root=$(git rev-parse --show-toplevel)
source=$root/hooks/pre-commit

# **Resolved to an absolute path, inside the `cd`.** `--git-common-dir` prints a path relative to
# the current directory — a bare `.git` from the root, `../.git` from a subdirectory — and a `cd`
# confined to a command substitution does not move the caller. Read relatively and used afterwards,
# it made `cd crates && ../hooks/install.sh` create `crates/.git/hooks/`, install nothing git would
# ever look at, and exit 0 saying it had succeeded.
#
# --git-common-dir, not --git-dir: in a worktree the two differ, and hooks live in the common one.
hooks=$(cd "$root" && cd "$(git rev-parse --git-common-dir)" && pwd)/hooks
target=$hooks/pre-commit

mkdir -p "$hooks"

# **Copied, not symlinked.** The hook directory is shared by the main checkout and every worktree,
# while `$root` is whichever tree ran this. A link into a worktree dies with `git worktree remove`,
# and git tests a hook with access(X_OK), which fails on a dangling link exactly as on a missing
# file — so every commit everywhere would silently run no hook, and re-running this script could not
# repair it either, because the guard below would see a path that is not `$source` and refuse.
#
# Linking to the main checkout's copy is the other durable answer and is not available: `hooks/`
# lives on a feature branch, so that link dangles whenever the main checkout is on another branch.
# The cost of copying is that editing hooks/pre-commit needs a re-install, which is what the last
# line prints.
#
# The guard reads a marker, not the content. Comparing content refused exactly the case the
# installer exists to serve: editing hooks/pre-commit is what makes source and target differ, so the
# re-install that edit calls for took the refusal branch and exited 1. A marker says "this hook is
# ours" whatever it now contains, and says nothing about anyone else's.
if [ -e "$target" ] && ! grep -q '^# laconic-hook: pre-commit$' "$target" 2>/dev/null; then
  echo "install: $target exists and is not laconic's — move it aside first" >&2
  exit 1
fi

# **Removed before copying.** `cp` writes *through* a symlink at the destination, so upgrading from
# the version of this script that used `ln -s` followed the link and wrote into the worktree it
# pointed at — leaving `.git/hooks/pre-commit` a symlink, reporting success, and keeping the
# dangling-link failure this copy exists to remove.
rm -f "$target"
cp "$source" "$target"
chmod +x "$target"
echo "installed $target"
echo "re-run this after editing hooks/pre-commit — the hook is a copy, not a link"
