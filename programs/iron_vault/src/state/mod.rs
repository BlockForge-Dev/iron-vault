use anchor_lang::prelude::*;

/// Closed state machine for a fixed-destination escrow.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, InitSpace, PartialEq, Eq)]
pub enum EscrowStatus {
    Funded,
    Released,
    Refunded,
}

/// Immutable escrow terms plus the single mutable lifecycle status.
#[account]
#[derive(Debug, InitSpace)]
pub struct Escrow {
    pub maker: Pubkey,
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub escrow_id: u64,
    pub amount: u64,
    pub created_at: i64,
    pub expires_at: i64,
    pub status: EscrowStatus,
    pub bump: u8,
    pub reserved: [u8; 30],
}

impl Escrow {
    /// Anchor discriminator plus the exact serialized Milestone 0 payload.
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

/// Authority and lifecycle configuration for one multi-asset vault.
#[account]
#[derive(Debug, InitSpace)]
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

impl Vault {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

/// Registration and future policy state for one mint held by a vault.
#[account]
#[derive(Debug, InitSpace)]
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

impl VaultAsset {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escrow_size_matches_the_account_model() {
        assert_eq!(Escrow::INIT_SPACE, 192);
        assert_eq!(Escrow::SPACE, 200);
    }

    #[test]
    fn vault_sizes_match_the_account_model() {
        assert_eq!(Vault::INIT_SPACE, 160);
        assert_eq!(Vault::SPACE, 168);
        assert_eq!(VaultAsset::INIT_SPACE, 192);
        assert_eq!(VaultAsset::SPACE, 200);
    }
}
