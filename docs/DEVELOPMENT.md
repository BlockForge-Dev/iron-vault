# Development and reproducible builds

## Pinned toolchain

| Tool | Version | Pin location |
|---|---:|---|
| Rust | 1.91.1 | `rust-toolchain.toml`, `.tool-versions` |
| Solana CLI / Agave | 3.1.10 | `.tool-versions`, `scripts/install-toolchain.sh` |
| Anchor CLI | 1.1.2 | `.tool-versions`, installer script |
| `anchor-lang` | 1.1.2 exact | workspace `Cargo.toml` |
| `anchor-spl` | 1.1.2 exact | workspace/program `Cargo.toml` |
| Node.js | 24.10.0 | `.nvmrc`, `package.json`, CI |
| pnpm | 11.23.0 | `packageManager`, `.tool-versions`, CI |
| LiteSVM | 0.13.1 exact | program dev dependencies |

`anchor-spl` has default features disabled and enables only `token` and
`token_2022`. Token-bearing instructions use `InterfaceAccount<Mint>`,
`InterfaceAccount<TokenAccount>`, and `Interface<TokenInterface>`. Each mint and
token account is additionally bound to the selected or stored token-program ID.
The v1 parser accepts legacy SPL Token and Token-2022 mints with an empty TLV
extension list. Every initialized Token-2022 mint extension is rejected before
funding or registration. See `docs/TOKEN_2022_POLICY.md`.

LiteSVM 0.13.1 is the patch line paired with the Anchor 1.1 template and Solana
3.1 client crates. Newer LiteSVM releases track newer Agave internals and require
an explicit compatibility upgrade rather than an unreviewed floating update.

## Clean checkout

Anchor supports Linux and macOS; on Windows use WSL. Install the ordinary system
build prerequisites and Rustup, then run:

```bash
git clone <repository-url> iron-vault
cd iron-vault
bash scripts/install-toolchain.sh
pnpm install --frozen-lockfile
make test
```

`make test` verifies versions, formatting, Clippy, host unit tests, SBF builds for
IronVault and the test-only mock multisig, the in-process LiteSVM suites, SDK
and CLI builds/tests, and offline upgrade-metadata regressions. `make ci` additionally runs
RustSec and pnpm dependency audits.

The SDK pins `@solana/web3.js`. Canonical associated-token derivation and its
six-account idempotent-create instruction are implemented locally and tested;
the broader `@solana/spl-token` package is intentionally excluded because its
current dependency graph contains an unpatched high-severity advisory. Optional
WebSocket native packages (`bufferutil` and `utf-8-validate`) have install scripts
disabled in `pnpm-workspace.yaml`; JavaScript fallbacks remain available.

## Test boundaries

LiteSVM is the fast deterministic execution gate for program instructions. It
does not prove validator/RPC-specific behavior, scheduling, or exact production
compute usage. The mock multisig proves generic external PDA-signer CPI semantics;
it does not prove a particular deployed Squads version or configuration. Live
upgrade authority and bytecode require the finalized-cluster procedure in
`docs/UPGRADE_POLICY.md`.

The committed `Cargo.lock` and `pnpm-lock.yaml` are authoritative dependency
resolutions. CI always uses locked/frozen modes. Updating a direct dependency
requires an intentional pin change, regenerated lockfiles, audit, and review.

RustSec currently reports upstream maintenance warnings in Anchor/Solana and the
LiteSVM dev graph, including `RUSTSEC-2026-0097` through LiteSVM's Agave syscall
emulation. It reports no known vulnerability advisory. CI runs `cargo audit` so
vulnerabilities fail while maintenance/unsoundness warnings remain visible. The
warnings are not runtime program dependencies except for Anchor's legacy
`bincode`; review them on every dependency update and do not expand this policy
into ignored vulnerability IDs.

`pnpm audit` reports one moderate advisory in `uuid@8.3.2`, reached through
`@solana/web3.js -> jayson`. The advisory concerns the v3/v5/v6 APIs when a
caller supplies an undersized output buffer; the pinned `jayson@4.3.0` source
invokes only `uuid.v4()` without a buffer. There is no patched `uuid` release
within Jayson's declared major range. CI still fails on high or critical npm
advisories, and this residual must be reevaluated when the Web3 pin changes.
