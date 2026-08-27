use {
    crate::{
        constants::{PAUSE_VAULT_CONFIG, PROTOCOL_SEED, VAULT_SEED},
        error::IronVaultError,
        events::VaultAuthorityUpdated,
        security::pause::require_protocol_active,
        state::{ProtocolConfig, Vault},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SetVaultAuthority<'info> {
    pub current_authority: Signer<'info>,
    #[account(seeds = [PROTOCOL_SEED], bump = protocol_config.bump)]
    pub protocol_config: Account<'info, ProtocolConfig>,
    #[account(
        mut,
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
        constraint = vault.authority == current_authority.key() @ IronVaultError::InvalidVaultAuthority,
    )]
    pub vault: Account<'info, Vault>,
}

pub fn set_authority(ctx: Context<SetVaultAuthority>, new_authority: Pubkey) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_VAULT_CONFIG)?;
    let previous_authority = ctx.accounts.vault.authority;
    require!(
        new_authority != Pubkey::default()
            && new_authority != previous_authority
            && new_authority != ctx.accounts.vault.guardian,
        IronVaultError::InvalidNewVaultAuthority
    );

    ctx.accounts.vault.authority = new_authority;
    emit!(VaultAuthorityUpdated {
        vault: ctx.accounts.vault.key(),
        previous_authority,
        new_authority,
    });
    Ok(())
}
