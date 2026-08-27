use {
    crate::{
        constants::{KNOWN_PAUSE_FLAGS, PROTOCOL_SEED},
        error::IronVaultError,
        events::ProtocolPauseUpdated,
        state::ProtocolConfig,
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SetProtocolPause<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_config.bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,
}

pub fn set_pause(ctx: Context<SetProtocolPause>, flags: u32) -> Result<()> {
    require!(
        flags & !KNOWN_PAUSE_FLAGS == 0,
        IronVaultError::InvalidPauseFlags
    );

    let config = &mut ctx.accounts.protocol_config;
    let caller = ctx.accounts.caller.key();
    require!(
        caller == config.admin || caller == config.guardian,
        IronVaultError::UnauthorizedProtocolPause
    );
    if caller == config.guardian {
        require!(
            flags & config.pause_flags == config.pause_flags,
            IronVaultError::GuardianCannotUnpause
        );
    }

    let previous_flags = config.pause_flags;
    config.pause_flags = flags;
    emit!(ProtocolPauseUpdated {
        version: crate::constants::INITIAL_SCHEMA_VERSION,
        protocol_config: config.key(),
        caller,
        previous_flags,
        new_flags: flags,
    });
    Ok(())
}
