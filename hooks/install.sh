#!/bin/sh
# Symlink hooks/pre-commit into this repository's hook directory.
#
# One file, by symlink, into the existing directory. Two things this deliberately does not do:
# set core.hooksPath, which replaces .git/hooks wholesale and would disable every hook installed
# there by anything else; and overwrite a pre-commit hook that is already present and not ours.
set -eu

root=$(git rev-parse --show-toplevel)
# --git-common-dir, not --git-dir: in a worktree the two differ, and hooks live in the common one.
hooks=$(cd "$root" && git rev-parse --git-common-dir)/hooks
source=$root/hooks/pre-commit
target=$hooks/pre-commit

mkdir -p "$hooks"
if [ -e "$target" ] && [ "$(readlink "$target" || true)" != "$source" ]; then
  echo "install: $target exists and is not ours — move it aside first" >&2
  exit 1
fi

ln -sf "$source" "$target"
chmod +x "$source"
echo "installed $target"
