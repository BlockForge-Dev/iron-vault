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

`anchor-spl` has default features disabled. Milestone 2 uses only the classic
SPL Token CPI surface and every escrow instruction requires `Program<Token>`, so
Token-2022 program and account inputs are rejected. Anchor 1.1.2's account-init
derive references shared Token-2022 interface helpers at compile time, which is
why that Cargo feature is enabled; it does not widen the accepted runtime token
program. The SDK has no runtime dependency yet; an Anchor client will be added
with the first transaction builder that exercises it.

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

`make test` verifies versions, formatting, Clippy, host unit tests, an SBF build,
the in-process LiteSVM escrow suite, and SDK tests. `make ci` additionally runs
RustSec and pnpm dependency audits.

## Test boundaries

LiteSVM is the fast deterministic execution gate for program instructions. It
does not prove validator/RPC-specific behavior, scheduling, or exact production
compute usage. Tests that require those semantics will use a pinned local
validator in a later milestone.

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
