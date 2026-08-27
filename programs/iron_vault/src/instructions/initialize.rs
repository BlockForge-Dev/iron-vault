use anchor_lang::prelude::*;

/// Account set for the Milestone 1 dispatch smoke test.
#[derive(Accounts)]
pub struct Initialize {}

/// Executes a side-effect-free program dispatch.
pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    msg!("IronVault scaffold active: {:?}", ctx.program_id);
    Ok(())
}
