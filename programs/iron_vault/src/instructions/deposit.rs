use {
    crate::{
        constants::{VAULT_ASSET_SEED, VAULT_SEED, VAULT_TOKEN_SEED},
        error::IronVaultError,
        events::VaultDeposit,
        state::{Vault, VaultAsset},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked},
};

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub depositor: Signer<'info>,
    #[account(
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [VAULT_ASSET_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump = vault_asset.bump,
        has_one = vault,
        has_one = mint,
        constraint = vault_asset.token_program == token_program.key(),
    )]
    pub vault_asset: Account<'info, VaultAsset>,
    #[account(
        mut,
        constraint = source_token.owner == depositor.key() @ IronVaultError::InvalidDepositSourceOwner,
        constraint = source_token.mint == vault_asset.mint @ IronVaultError::InvalidDepositSourceMint,
    )]
    pub source_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump,
        constraint = vault_token.owner == vault.key() @ IronVaultError::InvalidVaultCustodyBalance,
        constraint = vault_token.mint == vault_asset.mint @ IronVaultError::InvalidDepositSourceMint,
    )]
    pub vault_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn deposit_tokens(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require_gt!(amount, 0, IronVaultError::InvalidVaultAmount);
    require!(
        ctx.accounts.vault_asset.enabled,
        IronVaultError::VaultAssetDisabled
    );
    require_gte!(
        ctx.accounts.source_token.amount,
        amount,
        IronVaultError::InsufficientFunds
    );

    let source_before = ctx.accounts.source_token.amount;
    let custody_before = ctx.accounts.vault_token.amount;
    let custody_after = custody_before
        .checked_add(amount)
        .ok_or(IronVaultError::InvalidVaultCustodyBalance)?;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.source_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.vault_token.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.source_token.reload()?;
    ctx.accounts.vault_token.reload()?;
    require_eq!(
        ctx.accounts.source_token.amount,
        source_before - amount,
        IronVaultError::InvalidDepositSourceBalance
    );
    require_eq!(
        ctx.accounts.vault_token.amount,
        custody_after,
        IronVaultError::InvalidVaultCustodyBalance
    );

    emit!(VaultDeposit {
        vault: ctx.accounts.vault.key(),
        vault_asset: ctx.accounts.vault_asset.key(),
        vault_token: ctx.accounts.vault_token.key(),
        depositor: ctx.accounts.depositor.key(),
        source_token: ctx.accounts.source_token.key(),
        mint: ctx.accounts.mint.key(),
        amount,
    });

    Ok(())
}
