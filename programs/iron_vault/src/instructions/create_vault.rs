use {
    crate::{constants::VAULT_SEED, error::IronVaultError, events::VaultCreated, state::Vault},
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(vault_id: u64)]
pub struct CreateVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = Vault::SPACE,
        seeds = [VAULT_SEED, authority.key().as_ref(), vault_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

pub fn create_vault_account(
    ctx: Context<CreateVault>,
    vault_id: u64,
    guardian: Pubkey,
) -> Result<()> {
    let authority = ctx.accounts.authority.key();
    require!(
        guardian != Pubkey::default() && guardian != authority,
        IronVaultError::InvalidVaultGuardian
    );

    let vault = &mut ctx.accounts.vault;
    vault.namespace_authority = authority;
    vault.authority = authority;
    vault.guardian = guardian;
    vault.vault_id = vault_id;
    vault.next_withdrawal_id = 0;
    vault.paused = false;
    vault.bump = ctx.bumps.vault;
    vault.reserved = [0; 46];

    emit!(VaultCreated {
        vault: vault.key(),
        namespace_authority: authority,
        authority,
        guardian,
        vault_id,
    });

    Ok(())
}
