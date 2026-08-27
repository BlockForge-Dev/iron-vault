use {
    crate::{
        constants::{PAUSE_VAULT_CONFIG, PROTOCOL_SEED, ROLE_SEED, VAULT_SEED},
        error::IronVaultError,
        events::RoleRevoked,
        security::pause::require_protocol_active,
        state::{ProtocolConfig, RoleAssignment, Vault},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(principal: Pubkey)]
pub struct RevokeRole<'info> {
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
    #[account(
        mut,
        seeds = [ROLE_SEED, vault.key().as_ref(), principal.as_ref()],
        bump = role_assignment.bump,
        has_one = vault,
        constraint = role_assignment.principal == principal,
    )]
    pub role_assignment: Account<'info, RoleAssignment>,
}

pub fn revoke(ctx: Context<RevokeRole>, _principal: Pubkey) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_VAULT_CONFIG)?;
    let role = &mut ctx.accounts.role_assignment;
    require!(role.active, IronVaultError::RoleNotActive);
    let previous_permissions = role.permissions;
    role.permissions = 0;
    role.active = false;

    emit!(RoleRevoked {
        version: crate::constants::INITIAL_SCHEMA_VERSION,
        vault: role.vault,
        principal: role.principal,
        previous_permissions,
    });

    Ok(())
}
