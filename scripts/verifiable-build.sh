#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/programs/iron_vault"
anchor build --verifiable

artifact="$repo_root/target/verifiable/iron_vault.so"
if [[ ! -f "$artifact" ]]; then
  echo "Verifiable artifact not found: $artifact" >&2
  exit 1
fi
sha256sum "$artifact"
