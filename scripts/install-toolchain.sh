#!/usr/bin/env bash
set -euo pipefail

# Linux/WSL bootstrap. System build packages must already be present. CI
# installs Node first, while local environments receive the same pinned Node
# release under ~/.local/share when it is absent.
rustup toolchain install 1.91.1 --profile minimal --component clippy,rustfmt

node_version="24.10.0"
if ! command -v node >/dev/null 2>&1 || [[ "$(node --version)" != "v${node_version}" ]]; then
  case "$(uname -m)" in
    x86_64) node_arch="x64" ;;
    aarch64 | arm64) node_arch="arm64" ;;
    *)
      printf 'Unsupported Node architecture: %s\n' "$(uname -m)" >&2
      exit 1
      ;;
  esac

  node_archive="node-v${node_version}-linux-${node_arch}.tar.xz"
  node_root="$HOME/.local/share/node-v${node_version}-linux-${node_arch}"
  download_dir="$(mktemp -d)"
  trap 'rm -rf "$download_dir"' EXIT
  curl --proto '=https' --tlsv1.2 -sSfL \
    "https://nodejs.org/dist/v${node_version}/${node_archive}" \
    -o "$download_dir/$node_archive"
  curl --proto '=https' --tlsv1.2 -sSfL \
    "https://nodejs.org/dist/v${node_version}/SHASUMS256.txt" \
    -o "$download_dir/SHASUMS256.txt"
  (
    cd "$download_dir"
    grep "  ${node_archive}$" SHASUMS256.txt | sha256sum --check --strict
  )
  mkdir -p "$(dirname "$node_root")"
  tar -xJf "$download_dir/$node_archive" -C "$(dirname "$node_root")"
  rm -rf "$download_dir"
  trap - EXIT
fi

node_bin="$HOME/.local/share/node-v${node_version}-linux-${node_arch:-x64}/bin"
if [[ -d "$node_bin" ]]; then
  export PATH="$node_bin:$PATH"
  if [[ -n "${GITHUB_PATH:-}" ]]; then
    printf '%s\n' "$node_bin" >> "$GITHUB_PATH"
  fi
fi

if ! command -v solana >/dev/null 2>&1 || [[ "$(solana --version)" != *"3.1.10"* ]]; then
  installer="$(mktemp)"
  curl --proto '=https' --tlsv1.2 -sSfL https://release.anza.xyz/v3.1.10/install -o "$installer"
  sh "$installer"
  rm -f "$installer"
fi

solana_bin="$HOME/.local/share/solana/install/active_release/bin"
export PATH="$solana_bin:$PATH"
if [[ -n "${GITHUB_PATH:-}" ]]; then
  printf '%s\n' "$solana_bin" >> "$GITHUB_PATH"
fi

if ! command -v anchor >/dev/null 2>&1 || [[ "$(anchor --version)" != *"1.1.2"* ]]; then
  cargo +1.91.1 install --git https://github.com/otter-sec/anchor --tag v1.1.2 anchor-cli --locked
fi

npm install --global pnpm@11.23.0

if ! command -v cargo-audit >/dev/null 2>&1 || [[ "$(cargo audit --version)" != *"0.22.2"* ]]; then
  cargo +1.91.1 install cargo-audit --version 0.22.2 --locked
fi

bash scripts/check-toolchain.sh
