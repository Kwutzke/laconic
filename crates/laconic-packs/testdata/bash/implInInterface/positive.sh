#!/bin/sh
set -eu

_cache_dir=/tmp/cache

# Reads through _cache_dir when the entry is cold.
get() {
  cat "$1"
}
