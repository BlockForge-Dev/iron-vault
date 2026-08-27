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
