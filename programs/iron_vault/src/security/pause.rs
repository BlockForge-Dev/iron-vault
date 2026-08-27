use {
    crate::{error::IronVaultError, state::ProtocolConfig},
    anchor_lang::prelude::*,
};

pub fn require_protocol_active(config: &ProtocolConfig, flag: u32) -> Result<()> {
    require!(!config.is_paused(flag), IronVaultError::ProtocolPaused);
    Ok(())
}
