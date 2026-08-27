use {
    crate::{
        constants::{ROLE_SEED, VAULT_SEED},
        error::IronVaultError,
        events::RoleRevoked,
        state::{RoleAssignment, Vault},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(principal: Pubkey)]
pub struct RevokeRole<'info> {
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
    let role = &mut ctx.accounts.role_assignment;
    require!(role.active, IronVaultError::RoleNotActive);
    let previous_permissions = role.permissions;
    role.permissions = 0;
    role.active = false;

    emit!(RoleRevoked {
        vault: role.vault,
        principal: role.principal,
        previous_permissions,
    });

    Ok(())
}
