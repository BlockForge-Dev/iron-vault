use {
    crate::{
        constants::{PERMISSION_CANCEL_WITHDRAWAL, VAULT_SEED, WITHDRAWAL_SEED},
        error::IronVaultError,
        events::WithdrawalCancelled,
        security::permissions::validate_role_permission,
        state::{Vault, WithdrawalRequest, WithdrawalStatus},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CancelWithdrawal<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [VAULT_SEED, vault.namespace_authority.as_ref(), vault.vault_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Box<Account<'info, Vault>>,
    #[account(
        mut,
        seeds = [WITHDRAWAL_SEED, vault.key().as_ref(), withdrawal_request.withdrawal_id.to_le_bytes().as_ref()],
        bump = withdrawal_request.bump,
        has_one = vault,
    )]
    pub withdrawal_request: Box<Account<'info, WithdrawalRequest>>,
}

pub fn cancel(ctx: Context<CancelWithdrawal>) -> Result<()> {
    require!(
        ctx.accounts.withdrawal_request.status == WithdrawalStatus::Pending,
        IronVaultError::WithdrawalNotPending
    );
    authorize(&ctx)?;
    ctx.accounts.withdrawal_request.status = WithdrawalStatus::Cancelled;

    emit!(WithdrawalCancelled {
        vault: ctx.accounts.vault.key(),
        withdrawal_request: ctx.accounts.withdrawal_request.key(),
        caller: ctx.accounts.caller.key(),
        proposer: ctx.accounts.withdrawal_request.proposer,
        withdrawal_id: ctx.accounts.withdrawal_request.withdrawal_id,
    });
    Ok(())
}

fn authorize(ctx: &Context<CancelWithdrawal>) -> Result<()> {
    let caller = ctx.accounts.caller.key();
    if caller == ctx.accounts.vault.authority
        || caller == ctx.accounts.vault.guardian
        || caller == ctx.accounts.withdrawal_request.proposer
    {
        require!(
            ctx.remaining_accounts.is_empty(),
            IronVaultError::UnexpectedCancellationAccounts
        );
    } else {
        require!(
            ctx.remaining_accounts.len() <= 1,
            IronVaultError::UnexpectedCancellationAccounts
        );
        require_eq!(
            ctx.remaining_accounts.len(),
            1,
            IronVaultError::UnauthorizedWithdrawalCancellation
        );
        validate_role_permission(
            &ctx.remaining_accounts[0],
            &ctx.accounts.vault,
            caller,
            PERMISSION_CANCEL_WITHDRAWAL,
        )?;
    }
    Ok(())
}
