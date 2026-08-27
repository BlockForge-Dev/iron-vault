# ADR 0001: Use External Multisig Authorities

- Status: Accepted
- Scope: vault authority and program upgrade authority

## Decision

IronVault accepts ordinary Solana signer accounts and does not implement a
general-purpose multisig. Production vault and program-upgrade authorities are
external multisig-controlled PDAs. Program upgrade governance uses a distinct
2-of-3 multisig during development and mainnet maturation.

## Rationale

A custom multisig would add a second security-critical protocol whose membership,
threshold, replay protection, proposal lifecycle, and upgrade path would all
require separate review. Solana CPI signer semantics already let a reviewed
external multisig present its PDA as the signer IronVault expects.

## Verification

The repository includes a test-only 2-of-3 program. LiteSVM proves that duplicate
approvals, non-members, and the former vault authority fail, while two distinct
members can authorize a PDA CPI after authority rotation. Deployment verification
separately checks the finalized upgrade authority and deployed bytes.

## Consequences

- Multisig correctness and availability are external dependencies.
- The mock test program is evidence for signer interoperability only, not Squads
  compatibility or production multisig security.
- Revoking the loader upgrade authority makes the program permanently immutable;
  IronVault will retain multisig-controlled upgrades during maturation.
