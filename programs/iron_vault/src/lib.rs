pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod security;
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

    /// Initializes the singleton protocol administration and pause state.
    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>,
        admin: Pubkey,
        guardian: Pubkey,
    ) -> Result<()> {
        instructions::initialize_protocol::initialize_protocol_account(ctx, admin, guardian)
    }

    /// Replaces the protocol pause mask under admin or add-only guardian authority.
    pub fn set_protocol_pause(ctx: Context<SetProtocolPause>, flags: u32) -> Result<()> {
        instructions::set_protocol_pause::set_pause(ctx, flags)
    }

    /// Creates and atomically funds a fixed-destination supported-token escrow.
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

    /// Rotates vault authority without changing the stable vault PDA namespace.
    pub fn set_vault_authority(
        ctx: Context<SetVaultAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        instructions::set_vault_authority::set_authority(ctx, new_authority)
    }

    /// Pauses local vault outflows. The authority or guardian may call this.
    pub fn pause_vault(ctx: Context<PauseVault>) -> Result<()> {
        instructions::pause_vault::pause(ctx)
    }

    /// Restores local vault outflows. Only the vault authority may call this.
    pub fn unpause_vault(ctx: Context<UnpauseVault>) -> Result<()> {
        instructions::pause_vault::unpause(ctx)
    }

    /// Registers a supported mint and its canonical vault custody account.
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

    /// Updates one asset's per-transaction and rolling-window withdrawal limits.
    pub fn update_limits(
        ctx: Context<UpdateLimits>,
        max_per_transaction: u64,
        window_limit: u64,
        window_seconds: i64,
        timelock_threshold: u64,
        timelock_seconds: i64,
        request_execution_window_seconds: i64,
    ) -> Result<()> {
        instructions::update_limits::update(
            ctx,
            max_per_transaction,
            window_limit,
            window_seconds,
            timelock_threshold,
            timelock_seconds,
            request_execution_window_seconds,
        )
    }

    /// Creates an immutable timelocked withdrawal request.
    pub fn request_withdrawal(ctx: Context<RequestWithdrawal>, amount: u64) -> Result<()> {
        instructions::request_withdrawal::request(ctx, amount)
    }

    /// Permissionlessly executes a mature request to its immutable destination.
    pub fn execute_withdrawal(ctx: Context<ExecuteWithdrawal>) -> Result<()> {
        instructions::execute_withdrawal::execute(ctx)
    }

    /// Cancels a pending request under an intrinsic or delegated cancellation authority.
    pub fn cancel_withdrawal(ctx: Context<CancelWithdrawal>) -> Result<()> {
        instructions::cancel_withdrawal::cancel(ctx)
    }
}
