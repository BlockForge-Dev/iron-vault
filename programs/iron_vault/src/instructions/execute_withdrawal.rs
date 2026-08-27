use {
    crate::{
        constants::{
            PAUSE_VAULT_OUTFLOW, PROTOCOL_SEED, VAULT_ASSET_SEED, VAULT_SEED, VAULT_TOKEN_SEED,
            WITHDRAWAL_SEED,
        },
        error::IronVaultError,
        events::WithdrawalExecuted,
        instructions::withdraw::next_window_state,
        security::pause::require_protocol_active,
        state::{ProtocolConfig, Vault, VaultAsset, WithdrawalRequest, WithdrawalStatus},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked},
};

#[derive(Accounts)]
pub struct ExecuteWithdrawal<'info> {
    pub caller: Signer<'info>,
    #[account(seeds = [PROTOCOL_SEED], bump = protocol_config.bump)]
    pub protocol_config: Account<'info, ProtocolConfig>,
    #[account(
        seeds = [VAULT_SEED, vault.namespace_authority.as_ref(), vault.vault_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Box<Account<'info, Vault>>,
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [VAULT_ASSET_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump = vault_asset.bump,
        has_one = vault,
        has_one = mint,
        constraint = vault_asset.token_program == token_program.key(),
    )]
    pub vault_asset: Box<Account<'info, VaultAsset>>,
    #[account(
        mut,
        seeds = [WITHDRAWAL_SEED, vault.key().as_ref(), withdrawal_request.withdrawal_id.to_le_bytes().as_ref()],
        bump = withdrawal_request.bump,
        has_one = vault,
        has_one = vault_asset,
        has_one = mint,
        constraint = withdrawal_request.token_program == token_program.key(),
    )]
    pub withdrawal_request: Box<Account<'info, WithdrawalRequest>>,
    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump,
        constraint = vault_token.owner == vault.key() @ IronVaultError::InvalidVaultCustodyBalance,
        constraint = vault_token.mint == mint.key() @ IronVaultError::InvalidWithdrawalMint,
    )]
    pub vault_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        address = withdrawal_request.recipient_token_account @ IronVaultError::InvalidWithdrawalRecipient,
        constraint = recipient_token.owner == withdrawal_request.recipient_owner @ IronVaultError::InvalidWithdrawalRecipient,
        constraint = recipient_token.mint == withdrawal_request.mint @ IronVaultError::InvalidWithdrawalMint,
    )]
    pub recipient_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn execute(ctx: Context<ExecuteWithdrawal>) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_VAULT_OUTFLOW)?;
    let request = &ctx.accounts.withdrawal_request;
    require!(
        request.status == WithdrawalStatus::Pending,
        IronVaultError::WithdrawalNotPending
    );
    let now = ctx.accounts.clock.unix_timestamp;
    require_gte!(
        now,
        request.execute_after,
        IronVaultError::WithdrawalTimelockActive
    );
    require_gte!(
        request.expires_at,
        now,
        IronVaultError::WithdrawalRequestExpired
    );
    require!(!ctx.accounts.vault.paused, IronVaultError::VaultPaused);
    require!(
        ctx.accounts.vault_asset.enabled,
        IronVaultError::VaultAssetDisabled
    );
    require_gte!(
        ctx.accounts.vault_asset.max_per_transaction,
        request.amount,
        IronVaultError::PerTransactionLimitExceeded
    );
    require_gte!(
        ctx.accounts.vault_token.amount,
        request.amount,
        IronVaultError::InsufficientVaultFunds
    );

    let (next_window_start, next_window_spent) =
        next_window_state(&ctx.accounts.vault_asset, request.amount, now)?;
    let custody_before = ctx.accounts.vault_token.amount;
    let destination_before = ctx.accounts.recipient_token.amount;
    let destination_after = destination_before
        .checked_add(request.amount)
        .ok_or(IronVaultError::InvalidWithdrawalDestinationBalance)?;
    let vault_id = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = [ctx.accounts.vault.bump];
    let signer_seeds: &[&[u8]] = &[
        VAULT_SEED,
        ctx.accounts.vault.namespace_authority.as_ref(),
        &vault_id,
        &bump,
    ];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.recipient_token.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            &[signer_seeds],
        ),
        request.amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.vault_token.reload()?;
    ctx.accounts.recipient_token.reload()?;
    require_eq!(
        ctx.accounts.vault_token.amount,
        custody_before - request.amount,
        IronVaultError::InvalidVaultCustodyBalance
    );
    require_eq!(
        ctx.accounts.recipient_token.amount,
        destination_after,
        IronVaultError::InvalidWithdrawalDestinationBalance
    );
    ctx.accounts.vault_asset.window_started_at = next_window_start;
    ctx.accounts.vault_asset.window_spent = next_window_spent;
    ctx.accounts.withdrawal_request.status = WithdrawalStatus::Executed;

    emit!(WithdrawalExecuted {
        vault: ctx.accounts.vault.key(),
        vault_asset: ctx.accounts.vault_asset.key(),
        withdrawal_request: ctx.accounts.withdrawal_request.key(),
        caller: ctx.accounts.caller.key(),
        recipient_token_account: ctx.accounts.recipient_token.key(),
        mint: ctx.accounts.mint.key(),
        withdrawal_id: ctx.accounts.withdrawal_request.withdrawal_id,
        amount: ctx.accounts.withdrawal_request.amount,
    });

    Ok(())
}
