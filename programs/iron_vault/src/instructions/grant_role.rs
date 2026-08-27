use {
    crate::{
        constants::{KNOWN_PERMISSIONS, PAUSE_VAULT_CONFIG, PROTOCOL_SEED, ROLE_SEED, VAULT_SEED},
        error::IronVaultError,
        events::RoleGranted,
        security::pause::require_protocol_active,
        state::{ProtocolConfig, RoleAssignment, Vault},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(principal: Pubkey)]
pub struct GrantRole<'info> {
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
    #[account(
        init_if_needed,
        payer = authority,
        space = RoleAssignment::SPACE,
        seeds = [ROLE_SEED, vault.key().as_ref(), principal.as_ref()],
        bump,
    )]
    pub role_assignment: Account<'info, RoleAssignment>,
    pub system_program: Program<'info, System>,
}

pub fn grant(ctx: Context<GrantRole>, principal: Pubkey, permissions: u64) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_VAULT_CONFIG)?;
    require!(
        principal != Pubkey::default()
            && principal != ctx.accounts.vault.authority
            && principal != ctx.accounts.vault.guardian,
        IronVaultError::InvalidRolePrincipal
    );
    require!(
        permissions != 0 && permissions & !KNOWN_PERMISSIONS == 0,
        IronVaultError::InvalidPermissionMask
    );

    let role = &mut ctx.accounts.role_assignment;
    role.vault = ctx.accounts.vault.key();
    role.principal = principal;
    role.permissions = permissions;
    role.active = true;
    role.bump = ctx.bumps.role_assignment;
    role.reserved = [0; 54];

    emit!(RoleGranted {
        version: crate::constants::INITIAL_SCHEMA_VERSION,
        vault: role.vault,
        principal,
        permissions,
    });

    Ok(())
}
