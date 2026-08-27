use anchor_lang::prelude::*;

/// Emitted after an escrow and its custody account are atomically funded.
#[event]
pub struct EscrowCreated {
    pub escrow: Pubkey,
    pub escrow_token: Pubkey,
    pub maker: Pubkey,
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub created_at: i64,
    pub expires_at: i64,
    pub escrow_id: u64,
}

/// Emitted after a maker releases the exact escrow amount to its recipient.
#[event]
pub struct EscrowReleased {
    pub escrow: Pubkey,
    pub escrow_token: Pubkey,
    pub maker: Pubkey,
    pub recipient: Pubkey,
    pub recipient_token: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted after an expired escrow is returned to its immutable maker.
#[event]
pub struct EscrowRefunded {
    pub escrow: Pubkey,
    pub escrow_token: Pubkey,
    pub caller: Pubkey,
    pub maker: Pubkey,
    pub maker_destination: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted when an authority creates a new vault namespace.
#[event]
pub struct VaultCreated {
    pub vault: Pubkey,
    pub namespace_authority: Pubkey,
    pub authority: Pubkey,
    pub guardian: Pubkey,
    pub vault_id: u64,
}

/// Emitted when a classic SPL mint is registered with a vault.
#[event]
pub struct VaultAssetRegistered {
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub vault_token: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
}

/// Emitted after an exact permissionless deposit reaches vault custody.
#[event]
pub struct VaultDeposit {
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub vault_token: Pubkey,
    pub depositor: Pubkey,
    pub source_token: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted after the authority withdraws an exact amount from vault custody.
#[event]
pub struct VaultWithdrawal {
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub vault_token: Pubkey,
    pub caller: Pubkey,
    pub destination_token: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted when the vault authority creates or replaces an exact role mask.
#[event]
pub struct RoleGranted {
    pub vault: Pubkey,
    pub principal: Pubkey,
    pub permissions: u64,
}

/// Emitted when the vault authority immediately deactivates a role.
#[event]
pub struct RoleRevoked {
    pub vault: Pubkey,
    pub principal: Pubkey,
    pub previous_permissions: u64,
}

/// Emitted after an authorized caller updates one asset's withdrawal limits.
#[event]
pub struct VaultLimitsUpdated {
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub mint: Pubkey,
    pub caller: Pubkey,
    pub max_per_transaction: u64,
    pub window_limit: u64,
    pub window_seconds: i64,
    pub window_started_at: i64,
    pub window_spent: u64,
}
