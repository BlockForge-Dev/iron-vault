use {
    crate::{
        constants::{VAULT_ASSET_SEED, VAULT_SEED, VAULT_TOKEN_SEED},
        error::IronVaultError,
        events::VaultWithdrawal,
        state::{Vault, VaultAsset},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked},
};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
        constraint = vault.authority == authority.key() @ IronVaultError::InvalidVaultAuthority,
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
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump,
        constraint = vault_token.owner == vault.key() @ IronVaultError::InvalidVaultCustodyBalance,
        constraint = vault_token.mint == vault_asset.mint @ IronVaultError::InvalidWithdrawalDestinationMint,
    )]
    pub vault_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = destination_token.mint == vault_asset.mint @ IronVaultError::InvalidWithdrawalDestinationMint,
    )]
    pub destination_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn withdraw_tokens(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    require_gt!(amount, 0, IronVaultError::InvalidVaultAmount);
    require!(!ctx.accounts.vault.paused, IronVaultError::VaultPaused);
    require!(
        ctx.accounts.vault_asset.enabled,
        IronVaultError::VaultAssetDisabled
    );
    require_gte!(
        ctx.accounts.vault_token.amount,
        amount,
        IronVaultError::InsufficientVaultFunds
    );

    let custody_before = ctx.accounts.vault_token.amount;
    let destination_before = ctx.accounts.destination_token.amount;
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(IronVaultError::InvalidWithdrawalDestinationBalance)?;
    let vault_id = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = [ctx.accounts.vault.bump];
    let signer_seeds: &[&[u8]] = &[
        VAULT_SEED,
        ctx.accounts.vault.namespace_authority.as_ref(),
        &vault_id,
        &bump,
    ];
    let signer = &[signer_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.destination_token.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.vault_token.reload()?;
    ctx.accounts.destination_token.reload()?;
    require_eq!(
        ctx.accounts.vault_token.amount,
        custody_before - amount,
        IronVaultError::InvalidVaultCustodyBalance
    );
    require_eq!(
        ctx.accounts.destination_token.amount,
        destination_after,
        IronVaultError::InvalidWithdrawalDestinationBalance
    );

    emit!(VaultWithdrawal {
        vault: ctx.accounts.vault.key(),
        vault_asset: ctx.accounts.vault_asset.key(),
        vault_token: ctx.accounts.vault_token.key(),
        authority: ctx.accounts.authority.key(),
        destination_token: ctx.accounts.destination_token.key(),
        mint: ctx.accounts.mint.key(),
        amount,
    });

    Ok(())
}
