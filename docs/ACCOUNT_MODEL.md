# Account and PDA Model

This document fixes address derivation, ownership relationships, mutability, and
account sizing inputs for IronVault v1. Seed strings are ASCII bytes. Integers in
seeds are unsigned 64-bit little-endian values.

## 1. PDA registry

| Account | Seeds | Pays creation rent | Closed in v1 |
|---|---|---|---|
| `ProtocolConfig` | `[b"protocol"]` | initializer | No |
| `Escrow` | `[b"escrow", maker, escrow_id_le]` | maker | No; terminal tombstone |
| Escrow custody token account | `[b"escrow_token", escrow]` | maker | May close to maker only after terminal transfer |
| `Vault` | `[b"vault", namespace_authority, vault_id_le]` | authority | No |
| `VaultAsset` | `[b"vault_asset", vault, mint]` | registering signer | No |
| Vault custody token account | `[b"vault_token", vault, mint]` | registering signer | No |
| `RoleAssignment` | `[b"role", vault, principal]` | granting signer | No; revoked tombstone may be reused by replacement grant |
| `WithdrawalRequest` | `[b"withdrawal", vault, withdrawal_id_le]` | proposer | No; terminal tombstone |

Each state account stores its bump. Custody accounts use the corresponding state
PDA as token authority: `Escrow` for escrow custody and `Vault` for all vault
asset custody. The program signs token CPIs only with the exact canonical seeds
of that state PDA.

## 2. State layouts

Anchor's 8-byte discriminator is additional to every listed `SPACE` payload.
Future implementations MUST calculate and test exact serialized sizes; reserved
bytes are zeroed on creation and ignored by v1 readers.

### 2.1 ProtocolConfig

```rust
pub struct ProtocolConfig {
    pub version: u16,
    pub admin: Pubkey,
    pub guardian: Pubkey,
    pub pause_flags: u32,
    pub bump: u8,
    pub reserved: [u8; 61],
}
```

Known pause bits are `ESCROW_CREATE`, `ESCROW_RELEASE`, `VAULT_CONFIG`, and
`VAULT_OUTFLOW`. Unknown bits are rejected.

### 2.2 Escrow

```rust
pub struct Escrow {
    pub maker: Pubkey,
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub escrow_id: u64,
    pub amount: u64,
    pub created_at: i64,
    pub expires_at: i64,
    pub status: EscrowStatus, // serialized u8
    pub bump: u8,
    pub reserved: [u8; 30],
}
```

All fields except `status` are immutable. The account namespace includes maker
and ID; the account data independently stores and revalidates both.

### 2.3 Vault

```rust
pub struct Vault {
    pub namespace_authority: Pubkey,
    pub authority: Pubkey,
    pub guardian: Pubkey,
    pub vault_id: u64,
    pub next_withdrawal_id: u64,
    pub paused: bool,
    pub bump: u8,
    pub reserved: [u8; 46],
}
```

`namespace_authority` is immutable and only derives the stable PDA. `authority`
is mutable and is the sole authority identity after rotation.

### 2.4 VaultAsset

```rust
pub struct VaultAsset {
    pub vault: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub max_per_transaction: u64,
    pub window_limit: u64,
    pub window_seconds: i64,
    pub window_started_at: i64,
    pub window_spent: u64,
    pub timelock_threshold: u64,
    pub timelock_seconds: i64,
    pub request_execution_window_seconds: i64,
    pub enabled: bool,
    pub bump: u8,
    pub reserved: [u8; 30],
}
```

Only policy fields, enabled state, and window accounting are mutable. Vault,
mint, token program, and bump are immutable.

### 2.5 RoleAssignment

```rust
pub struct RoleAssignment {
    pub vault: Pubkey,
    pub principal: Pubkey,
    pub permissions: u64,
    pub active: bool,
    pub bump: u8,
    pub reserved: [u8; 54],
}
```

Vault and principal are immutable. A grant replaces the complete mask; it does
not merge permissions implicitly.

### 2.6 WithdrawalRequest

```rust
pub struct WithdrawalRequest {
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub proposer: Pubkey,
    pub recipient_owner: Pubkey,
    pub recipient_token_account: Pubkey,
    pub withdrawal_id: u64,
    pub amount: u64,
    pub created_at: i64,
    pub execute_after: i64,
    pub expires_at: i64,
    pub status: WithdrawalStatus, // serialized u8
    pub bump: u8,
    pub reserved: [u8; 30],
}
```

Only status is mutable. Storing both recipient owner and exact token account
prevents destination substitution and makes intent visible.

## 3. Token-account constraints

For every token CPI, Anchor constraints and handler checks jointly establish:

- mint account owner equals the stored token program;
- token account owner-program equals the stored token program;
- token-account mint equals the stored mint;
- source authority is a required signer for inbound transfers;
- custody authority equals the canonical IronVault state PDA;
- outbound destination equals the fixed stored request account, or its token
  owner equals the immutable escrow beneficiary as specified;
- custody accounts have no delegate, close authority, or unsupported extension;
- checked token transfer semantics are used, including mint decimals where the
  selected token interface requires them.

Associated token accounts are not mandatory. This permits beneficiaries to use
any valid fixed token account while retaining explicit owner/mint validation.

## 4. Relationships

```text
ProtocolConfig

Escrow 1 --- 1 Escrow custody token account

Vault 1 --- * VaultAsset 1 --- 1 Vault custody token account
  |                 |
  |                 +--- * WithdrawalRequest
  +--- * RoleAssignment
```

Every child stores its parent key and is additionally constrained by parent-based
PDA seeds. A caller cannot combine an asset, role, request, or custody account
from another vault even if mint and signer happen to match.

## 5. Initialization and mutation matrix

| Account | Created by | Mutable fields | Token authority |
|---|---|---|---|
| ProtocolConfig | `initialize_protocol` | admin, guardian, pause flags | None |
| Escrow | `create_escrow` | status only | Signs for escrow custody |
| Vault | `create_vault` | authority, guardian, next ID, paused | Signs for all vault custody |
| VaultAsset | `register_asset` | policy, enabled, window fields | None |
| RoleAssignment | `grant_role` | permissions, active | None |
| WithdrawalRequest | `request_withdrawal` | status only | None |

No instruction accepts a generic writable program-owned account where a typed,
seed-constrained account is expected. Accounts not documented as mutable MUST be
read-only in the transaction account metadata.
