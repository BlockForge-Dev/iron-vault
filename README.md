# IronVault

IronVault is a specification-first protocol for fixed-destination escrow and
policy-controlled token vaults on Solana. The intended implementation language
is Rust using Anchor.

The repository contains the Milestone 0 specification, reproducible Milestone 1
toolchain, and Milestone 2 classic SPL Token escrow implementation:

- [Protocol specification](docs/PROTOCOL_SPEC.md)
- [Account and PDA model](docs/ACCOUNT_MODEL.md)
- [State machines](docs/STATE_MACHINES.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Development and reproducible builds](docs/DEVELOPMENT.md)

## Quick verification

On Linux or WSL with the pinned toolchain installed:

```bash
pnpm install --frozen-lockfile
make test
```

The words MUST, MUST NOT, SHOULD, and MAY are normative. A future implementation
must conform to these documents or change the specification through a reviewed
decision record.

## Security status

Milestone 2 implements only fixed-destination `create_escrow` and
maker-authorized `release_escrow` for the classic SPL Token program. Refunds,
protocol pause controls, Token-2022, policy vaults, and multisig governance are
not implemented. The code has not been deployed or independently audited and
MUST NOT be represented as safe for arbitrary third-party mainnet funds.
