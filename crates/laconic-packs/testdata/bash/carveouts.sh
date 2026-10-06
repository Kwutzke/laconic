#!/usr/bin/env bash

# shellcheck disable=SC2034

# shellcheck source=./lib.sh

MAX_DEPTH=3

# Prints the depth limit to standard output.
exported() {
  echo "$MAX_DEPTH"
}

_unexported() {
  :
}
