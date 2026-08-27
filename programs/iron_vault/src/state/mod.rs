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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escrow_size_matches_the_account_model() {
        assert_eq!(Escrow::INIT_SPACE, 192);
        assert_eq!(Escrow::SPACE, 200);
    }
}
