use {
    crate::{
        constants::VAULT_SEED, error::IronVaultError, events::VaultPauseUpdated, state::Vault,
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct PauseVault<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,
}

#[derive(Accounts)]
pub struct UnpauseVault<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
        constraint = vault.authority == authority.key() @ IronVaultError::UnauthorizedVaultUnpause,
    )]
    pub vault: Account<'info, Vault>,
}

pub fn pause(ctx: Context<PauseVault>) -> Result<()> {
    let caller = ctx.accounts.caller.key();
    require!(
        caller == ctx.accounts.vault.authority || caller == ctx.accounts.vault.guardian,
        IronVaultError::UnauthorizedVaultPause
    );
    require!(
        !ctx.accounts.vault.paused,
        IronVaultError::VaultPauseUnchanged
    );
    ctx.accounts.vault.paused = true;
    emit!(VaultPauseUpdated {
        vault: ctx.accounts.vault.key(),
        caller,
        paused: true,
    });
    Ok(())
}

pub fn unpause(ctx: Context<UnpauseVault>) -> Result<()> {
    require!(
        ctx.accounts.vault.paused,
        IronVaultError::VaultPauseUnchanged
    );
    ctx.accounts.vault.paused = false;
    emit!(VaultPauseUpdated {
        vault: ctx.accounts.vault.key(),
        caller: ctx.accounts.authority.key(),
        paused: false,
    });
    Ok(())
}
