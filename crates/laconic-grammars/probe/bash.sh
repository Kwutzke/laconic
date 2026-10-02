#!/usr/bin/env bash
# Documents the file.

# a line comment

# shellcheck disable=SC2034
MAX_DEPTH=3

# Documents an exported function.
exported() {
  local x=$(($1 + 1))
  echo "$x # not a comment" # a trailing comment

  # a detached comment

  return "$x"
}

function _unexported {
  cat <<EOT
# not a comment either
EOT
}
