#!/usr/bin/env bash
set -euo pipefail

expect_version() {
  local label="$1"
  local expected="$2"
  shift 2
  local actual
  actual="$("$@" 2>&1)"
  if [[ "$actual" != *"$expected"* ]]; then
    printf '%s version mismatch: expected %s, got %s\n' "$label" "$expected" "$actual" >&2
    return 1
  fi
  printf '%-8s %s\n' "$label" "$actual"
}

expect_version Rust 1.91.1 rustc --version
expect_version Solana 3.1.10 solana --version
expect_version Anchor 1.1.2 anchor --version
expect_version Node 24.10.0 node --version
expect_version pnpm 11.23.0 pnpm --version
