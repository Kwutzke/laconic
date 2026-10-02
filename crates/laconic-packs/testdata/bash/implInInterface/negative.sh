#!/bin/sh
set -eu

cache_dir=/tmp/cache

# Reads through cache_dir when the entry is cold.
get() {
  cat "$1"
}
