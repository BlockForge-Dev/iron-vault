use {
    crate::{
        constants::{
            PAUSE_VAULT_CONFIG, PROTOCOL_SEED, VAULT_ASSET_SEED, VAULT_SEED, VAULT_TOKEN_SEED,
        },
        error::IronVaultError,
        events::VaultAssetRegistered,
        security::pause::require_protocol_active,
        state::{ProtocolConfig, Vault, VaultAsset},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
pub struct RegisterAsset<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [PROTOCOL_SEED], bump = protocol_config.bump)]
    pub protocol_config: Account<'info, ProtocolConfig>,
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
        init,
        payer = authority,
        space = VaultAsset::SPACE,
        seeds = [VAULT_ASSET_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub vault_asset: Account<'info, VaultAsset>,
    #[account(
        init,
        payer = authority,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = vault,
        token::token_program = token_program,
    )]
    pub vault_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn register_vault_asset(ctx: Context<RegisterAsset>) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_VAULT_CONFIG)?;
    let asset = &mut ctx.accounts.vault_asset;
    asset.vault = ctx.accounts.vault.key();
    asset.mint = ctx.accounts.mint.key();
    asset.token_program = ctx.accounts.token_program.key();
    asset.max_per_transaction = u64::MAX;
    asset.window_limit = u64::MAX;
    asset.window_seconds = 0;
    asset.window_started_at = Clock::get()?.unix_timestamp;
    asset.window_spent = 0;
    asset.timelock_threshold = u64::MAX;
    asset.timelock_seconds = 0;
    asset.request_execution_window_seconds = 0;
    asset.enabled = true;
    asset.bump = ctx.bumps.vault_asset;
    asset.reserved = [0; 30];

    emit!(VaultAssetRegistered {
        vault: ctx.accounts.vault.key(),
        vault_asset: asset.key(),
        vault_token: ctx.accounts.vault_token.key(),
        mint: asset.mint,
        token_program: asset.token_program,
    });

    Ok(())
}
