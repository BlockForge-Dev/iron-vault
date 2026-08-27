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

    /// Returns a funded escrow to its maker at or after expiry.
    ///
    /// Any signer may trigger the refund, but cannot redirect its destination.
    pub fn refund_escrow(ctx: Context<RefundEscrow>) -> Result<()> {
        instructions::refund_escrow::refund(ctx)
    }

    /// Creates a stable vault namespace controlled by one authority.
    pub fn create_vault(ctx: Context<CreateVault>, vault_id: u64, guardian: Pubkey) -> Result<()> {
        instructions::create_vault::create_vault_account(ctx, vault_id, guardian)
    }

    /// Registers a classic SPL mint and its canonical vault custody account.
    pub fn register_asset(ctx: Context<RegisterAsset>) -> Result<()> {
        instructions::register_asset::register_vault_asset(ctx)
    }

    /// Deposits an exact positive amount into a registered vault asset.
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::deposit_tokens(ctx, amount)
    }

    /// Withdraws an exact amount under the stored vault authority's signature.
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        instructions::withdraw::withdraw_tokens(ctx, amount)
    }

    /// Creates, replaces, or reactivates an exact role permission mask.
    pub fn grant_role(ctx: Context<GrantRole>, principal: Pubkey, permissions: u64) -> Result<()> {
        instructions::grant_role::grant(ctx, principal, permissions)
    }

    /// Immediately clears and deactivates a role assignment.
    pub fn revoke_role(ctx: Context<RevokeRole>, principal: Pubkey) -> Result<()> {
        instructions::revoke_role::revoke(ctx, principal)
    }
}
