use {
    crate::{
        constants::{INITIAL_SCHEMA_VERSION, PROTOCOL_SEED},
        error::IronVaultError,
        events::ProtocolInitialized,
        state::ProtocolConfig,
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        init,
        payer = initializer,
        space = ProtocolConfig::SPACE,
        seeds = [PROTOCOL_SEED],
        bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_protocol_account(
    ctx: Context<InitializeProtocol>,
    admin: Pubkey,
    guardian: Pubkey,
) -> Result<()> {
    require!(
        admin != Pubkey::default() && guardian != Pubkey::default() && admin != guardian,
        IronVaultError::InvalidProtocolAuthority
    );

    let config = &mut ctx.accounts.protocol_config;
    config.version = INITIAL_SCHEMA_VERSION;
    config.admin = admin;
    config.guardian = guardian;
    config.pause_flags = 0;
    config.bump = ctx.bumps.protocol_config;
    config.reserved = [0; 61];

    emit!(ProtocolInitialized {
        protocol_config: config.key(),
        admin,
        guardian,
        version: INITIAL_SCHEMA_VERSION,
    });
    Ok(())
}
