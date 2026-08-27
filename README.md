# IronVault

IronVault is a specification-first protocol for fixed-destination escrow and
policy-controlled token vaults on Solana. The intended implementation language
is Rust using Anchor.

The repository contains the Milestone 0 specification, reproducible Milestone 1
toolchain, IronVault v0.1 classic SPL Token escrow lifecycle, and a multi-asset
vault core with role permissions, withdrawal limits, timelocked withdrawals,
and directional emergency controls:

- [Protocol specification](docs/PROTOCOL_SPEC.md)
- [Account and PDA model](docs/ACCOUNT_MODEL.md)
- [State machines](docs/STATE_MACHINES.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Upgrade and external multisig policy](docs/UPGRADE_POLICY.md)
- [Development and reproducible builds](docs/DEVELOPMENT.md)

## Quick verification

On Linux or WSL with the pinned toolchain installed:

```bash
pnpm install --frozen-lockfile
make test
```

The words MUST, MUST NOT, SHOULD, and MAY are normative. The implementation must
conform to these documents or change the specification through a reviewed
decision record.

## Security status

IronVault v0.1 implements fixed-destination `create_escrow`, maker-authorized
`release_escrow` before expiry, and permissionless `refund_escrow` at or after
expiry for the classic SPL Token program. Refund destinations are constrained
to an account owned by the immutable maker and using the immutable escrow mint.
The vault supports creation, authority-only asset registration, permissionless
deposits into canonical per-mint custody PDAs, and withdrawals by the authority
or an active canonical role carrying `WITHDRAW`. Role grants use exact masks and
revocation takes effect on the next call. Authorities or active `MANAGE_LIMITS`
roles can configure positive per-transaction and rolling-window limits
independently for every registered asset. Successful withdrawals atomically
consume window capacity using checked arithmetic. Amounts above an asset's
instant threshold require an immutable, fixed-recipient withdrawal request.
Authority or `REQUEST_WITHDRAWAL` operators can propose one; execution is
permissionless only during its stored execution window, and later policy changes
cannot shorten that request's original timelock. The authority, guardian,
proposer, or an active `CANCEL_WITHDRAWAL` operator can cancel a pending request.
The protocol admin and add-only guardian control separate escrow-creation,
escrow-release, vault-configuration, and vault-outflow pause flags. A vault
authority or guardian may pause local outflows, but only the authority may
unpause. Deposits, cancellation, and expired escrow refunds remain available
during their relevant pauses. An authority address may be controlled by an
external multisig, but embedded multisig governance and Token-2022 are not
implemented. A test-only external 2-of-3 CPI harness validates the signer
abstraction, and a read-only deployment verifier checks finalized upgrade
authority and bytecode. This does not prove any live Squads configuration. The
code has not been deployed or independently audited and MUST NOT be represented
as safe for arbitrary third-party mainnet funds.
