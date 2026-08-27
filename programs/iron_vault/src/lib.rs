pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1");

#[program]
pub mod iron_vault {
    use super::*;

    /// Confirms that the scaffolded program can be dispatched in an SVM.
    ///
    /// This intentionally creates no protocol state. Milestone 1 proves the
    /// repository and toolchain; protocol initialization begins in a later
    /// implementation milestone.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::handler(ctx)
    }
}
