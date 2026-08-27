pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1");

#[program]
pub mod iron_vault {
    use super::*;

    /// Confirms that the scaffolded program can be dispatched in an SVM.
    ///
    /// This intentionally creates no protocol state. Milestone 1 proves the
    /// repository and toolchain; protocol initialization begins in a later
    /// implementation milestone.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::dispatch(ctx)
    }

    /// Creates and atomically funds a fixed-destination classic SPL escrow.
    pub fn create_escrow(
        ctx: Context<CreateEscrow>,
        escrow_id: u64,
        recipient: Pubkey,
        amount: u64,
        expires_at: i64,
    ) -> Result<()> {
        instructions::create_escrow::create(ctx, escrow_id, recipient, amount, expires_at)
    }

    /// Releases a funded, unexpired escrow to its immutable recipient.
    pub fn release_escrow(ctx: Context<ReleaseEscrow>) -> Result<()> {
        instructions::release_escrow::release(ctx)
    }
}
