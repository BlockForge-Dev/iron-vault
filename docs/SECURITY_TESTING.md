# Security testing campaign

IronVault uses four distinct evidence layers. Passing one layer MUST NOT be
represented as passing another.

| Layer | What it proves | Current gate |
|---|---|---|
| Pure Rust | arithmetic, permission helpers, state layouts, event decoding | `cargo test --workspace --lib --locked` and `make observer` |
| LiteSVM | Solana instruction execution and adversarial account substitution | `make litesvm` |
| Local validator | real RPC, signatures, deployment, logs, observer ingestion | `make e2e` |
| Devnet E2E | behavior of an actual public-cluster deployment | explicit deployment procedure; not an automatic local claim |

The LiteSVM suites already exercise invalid source owners/mints, fake token
programs, fixed recipient enforcement, release/refund replay, cross-vault asset
substitution, unauthorized roles, guardian withdrawal attempts, checked limit
overflow, timelock enforcement, immutable request terms, pause asymmetry, and
real extension-bearing Token-2022 mint rejection. Anchor constraints bind
signers, owners, token programs, mints, authorities, `has_one` relationships,
PDA seeds, and bumps; handler checks cover policy that cannot be expressed as a
declarative account constraint.

The guiding rule is: an account name supplied by a caller is not evidence.
Every authority, namespace, mint, token program, PDA, and state transition must
be proven by constraints or explicit checked logic.

Future security releases SHOULD add fuzz/property coverage for pure policy
helpers and preserve named adversarial regressions for every reported issue.
Devnet evidence must record cluster, finalized transaction signatures, program
hash, program-data address, upgrade authority, commit, and exact test command.
