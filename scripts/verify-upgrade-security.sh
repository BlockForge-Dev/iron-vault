#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/verify-upgrade-security.sh \
    --url <RPC_URL_OR_MONIKER> \
    --program-id <PROGRAM_ID> \
    --expected-upgrade-authority <MULTISIG_PDA|immutable> \
    --binary <LOCAL_PROGRAM_SO>

This command is read-only. It verifies finalized program metadata, the exact
upgrade authority, and a byte-for-byte local/on-chain program match.
EOF
}

rpc_url=""
program_id=""
expected_authority=""
local_binary=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --url) rpc_url="${2:-}"; shift 2 ;;
    --program-id) program_id="${2:-}"; shift 2 ;;
    --expected-upgrade-authority) expected_authority="${2:-}"; shift 2 ;;
    --binary) local_binary="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$rpc_url" || -z "$program_id" || -z "$expected_authority" || -z "$local_binary" ]]; then
  usage >&2
  exit 2
fi
if [[ ! -f "$local_binary" ]]; then
  printf 'Local program binary does not exist: %s\n' "$local_binary" >&2
  exit 1
fi
for command_name in solana node sha256sum cmp mktemp awk; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' "$command_name" >&2
    exit 1
  fi
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
metadata_json="$(solana program show \
  --url "$rpc_url" \
  --commitment finalized \
  --output json \
  "$program_id")"
printf '%s' "$metadata_json" | node "$script_dir/verify-program-metadata.mjs" \
  "$program_id" "$expected_authority"

deployed_binary="$(mktemp "${TMPDIR:-/tmp}/iron-vault-deployed.XXXXXX.so")"
trap 'rm -f "$deployed_binary"' EXIT
solana program dump \
  --url "$rpc_url" \
  --commitment finalized \
  "$program_id" \
  "$deployed_binary" >/dev/null

local_hash="$(sha256sum "$local_binary" | awk '{print $1}')"
deployed_hash="$(sha256sum "$deployed_binary" | awk '{print $1}')"
if ! cmp -s "$local_binary" "$deployed_binary"; then
  printf 'Deployed bytecode mismatch.\nLocal SHA-256:    %s\nOn-chain SHA-256: %s\n' \
    "$local_hash" "$deployed_hash" >&2
  exit 1
fi

printf 'Upgrade security verification passed.\nProgram: %s\nAuthority: %s\nSHA-256: %s\n' \
  "$program_id" "$expected_authority" "$local_hash"
