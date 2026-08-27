# IronVault

IronVault is a specification-first protocol for fixed-destination escrow and
policy-controlled token vaults on Solana. The intended implementation language
is Rust using Anchor.

The repository contains the Milestone 0 specification, reproducible Milestone 1
toolchain, IronVault v0.1 classic SPL Token escrow lifecycle, and a multi-asset
vault core with Milestone 5 role permissions:

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

IronVault v0.1 implements fixed-destination `create_escrow`, maker-authorized
`release_escrow` before expiry, and permissionless `refund_escrow` at or after
expiry for the classic SPL Token program. Refund destinations are constrained
to an account owned by the immutable maker and using the immutable escrow mint.
The vault supports creation, authority-only asset registration, permissionless
deposits into canonical per-mint custody PDAs, and withdrawals by the authority
or an active canonical role carrying `WITHDRAW`. Role grants use exact masks and
revocation takes effect on the next call. The other reserved permission bits are
stored but do not become operational until their corresponding instructions are
implemented. Asset limits initialize to unrestricted core defaults; enforced
withdrawal limits, timelocks, pause controls, Token-2022, and multisig governance
are not implemented. The code has not been deployed or independently audited
and MUST NOT be represented as safe for arbitrary third-party mainnet funds.
