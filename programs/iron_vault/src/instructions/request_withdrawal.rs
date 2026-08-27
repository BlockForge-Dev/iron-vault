use {
    crate::{
        constants::{PERMISSION_REQUEST_WITHDRAWAL, VAULT_ASSET_SEED, VAULT_SEED, WITHDRAWAL_SEED},
        error::IronVaultError,
        events::WithdrawalRequested,
        security::permissions::validate_role_permission,
        state::{Vault, VaultAsset, WithdrawalRequest, WithdrawalStatus},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, TokenAccount},
};

#[derive(Accounts)]
pub struct RequestWithdrawal<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,
    #[account(
        mut,
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
    )]
    pub vault: Box<Account<'info, Vault>>,
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [VAULT_ASSET_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump = vault_asset.bump,
        has_one = vault,
        has_one = mint,
    )]
    pub vault_asset: Box<Account<'info, VaultAsset>>,
    #[account(
        constraint = recipient_token.mint == mint.key() @ IronVaultError::InvalidWithdrawalMint,
    )]
    pub recipient_token: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = proposer,
        space = WithdrawalRequest::SPACE,
        seeds = [
            WITHDRAWAL_SEED,
            vault.key().as_ref(),
            vault.next_withdrawal_id.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub withdrawal_request: Box<Account<'info, WithdrawalRequest>>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn request(ctx: Context<RequestWithdrawal>, amount: u64) -> Result<()> {
    authorize(&ctx)?;
    require_gt!(amount, 0, IronVaultError::InvalidVaultAmount);
    require!(!ctx.accounts.vault.paused, IronVaultError::VaultPaused);
    require!(
        ctx.accounts.vault_asset.enabled,
        IronVaultError::VaultAssetDisabled
    );
    require_gt!(
        amount,
        ctx.accounts.vault_asset.timelock_threshold,
        IronVaultError::TimelockNotRequired
    );
    require_gte!(
        ctx.accounts.vault_asset.max_per_transaction,
        amount,
        IronVaultError::PerTransactionLimitExceeded
    );

    let now = ctx.accounts.clock.unix_timestamp;
    let execute_after = now
        .checked_add(ctx.accounts.vault_asset.timelock_seconds)
        .ok_or(IronVaultError::WithdrawalTimingOverflow)?;
    let expires_at = execute_after
        .checked_add(ctx.accounts.vault_asset.request_execution_window_seconds)
        .ok_or(IronVaultError::WithdrawalTimingOverflow)?;
    require_gt!(execute_after, now, IronVaultError::InvalidWithdrawalPolicy);
    require_gt!(
        expires_at,
        execute_after,
        IronVaultError::InvalidWithdrawalPolicy
    );

    let withdrawal_id = ctx.accounts.vault.next_withdrawal_id;
    ctx.accounts.vault.next_withdrawal_id = withdrawal_id
        .checked_add(1)
        .ok_or(IronVaultError::WithdrawalPolicyOverflow)?;

    let request = &mut ctx.accounts.withdrawal_request;
    request.vault = ctx.accounts.vault.key();
    request.vault_asset = ctx.accounts.vault_asset.key();
    request.mint = ctx.accounts.mint.key();
    request.token_program = ctx.accounts.vault_asset.token_program;
    request.proposer = ctx.accounts.proposer.key();
    request.recipient_owner = ctx.accounts.recipient_token.owner;
    request.recipient_token_account = ctx.accounts.recipient_token.key();
    request.withdrawal_id = withdrawal_id;
    request.amount = amount;
    request.created_at = now;
    request.execute_after = execute_after;
    request.expires_at = expires_at;
    request.status = WithdrawalStatus::Pending;
    request.bump = ctx.bumps.withdrawal_request;
    request.reserved = [0; 30];

    emit!(WithdrawalRequested {
        vault: request.vault,
        vault_asset: request.vault_asset,
        withdrawal_request: request.key(),
        proposer: request.proposer,
        recipient_owner: request.recipient_owner,
        recipient_token_account: request.recipient_token_account,
        mint: request.mint,
        withdrawal_id,
        amount,
        created_at: now,
        execute_after,
        expires_at,
    });

    Ok(())
}

fn authorize(ctx: &Context<RequestWithdrawal>) -> Result<()> {
    if ctx.accounts.proposer.key() == ctx.accounts.vault.authority {
        require!(
            ctx.remaining_accounts.is_empty(),
            IronVaultError::UnexpectedRequestAccounts
        );
    } else {
        require!(
            ctx.remaining_accounts.len() <= 1,
            IronVaultError::UnexpectedRequestAccounts
        );
        require_eq!(
            ctx.remaining_accounts.len(),
            1,
            IronVaultError::MissingVaultPermission
        );
        validate_role_permission(
            &ctx.remaining_accounts[0],
            &ctx.accounts.vault,
            ctx.accounts.proposer.key(),
            PERMISSION_REQUEST_WITHDRAWAL,
        )?;
    }
    Ok(())
}
