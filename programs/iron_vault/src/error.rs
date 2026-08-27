use anchor_lang::prelude::*;

/// Errors shared by future IronVault instructions.
#[error_code]
pub enum IronVaultError {
    /// The supplied protocol version is unsupported.
    #[msg("Unsupported protocol version")]
    UnsupportedVersion,
}
