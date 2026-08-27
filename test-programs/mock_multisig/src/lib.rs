use anchor_lang::{prelude::*, solana_program::program::invoke_signed};

declare_id!("Bo8iHtbsaLxRWrb39sZipzrNywzbxFXjXQQBYZXsKJc1");

const MULTISIG_SEED: &[u8] = b"multisig";

#[program]
pub mod mock_multisig {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, owners: [Pubkey; 3]) -> Result<()> {
        require!(
            owners.iter().all(|owner| *owner != Pubkey::default())
                && owners[0] != owners[1]
                && owners[0] != owners[2]
                && owners[1] != owners[2],
            MockMultisigError::InvalidOwners
        );

        let multisig = &mut ctx.accounts.multisig;
        multisig.creator = ctx.accounts.creator.key();
        multisig.owners = owners;
        multisig.threshold = 2;
        multisig.bump = ctx.bumps.multisig;
        Ok(())
    }

    pub fn execute<'info>(
        ctx: Context<'info, Execute<'info>>,
        instruction_data: Vec<u8>,
    ) -> Result<()> {
        let signer_a = ctx.accounts.signer_a.key();
        let signer_b = ctx.accounts.signer_b.key();
        require_keys_neq!(signer_a, signer_b, MockMultisigError::DuplicateApproval);
        require!(
            ctx.accounts.multisig.owners.contains(&signer_a)
                && ctx.accounts.multisig.owners.contains(&signer_b),
            MockMultisigError::NotMember
        );
        require_eq!(
            ctx.accounts.multisig.threshold,
            2,
            MockMultisigError::InvalidThreshold
        );

        let multisig_key = ctx.accounts.multisig.key();
        let account_metas = ctx
            .remaining_accounts
            .iter()
            .map(|account| AccountMeta {
                pubkey: *account.key,
                is_signer: *account.key == multisig_key,
                is_writable: account.is_writable,
            })
            .collect();
        let instruction = anchor_lang::solana_program::instruction::Instruction {
            program_id: ctx.accounts.target_program.key(),
            accounts: account_metas,
            data: instruction_data,
        };
        let creator = ctx.accounts.multisig.creator;
        let bump = [ctx.accounts.multisig.bump];
        let signer_seeds: &[&[u8]] = &[MULTISIG_SEED, creator.as_ref(), &bump];
        let mut account_infos = ctx.remaining_accounts.to_vec();
        account_infos.push(ctx.accounts.target_program.to_account_info());
        invoke_signed(&instruction, &account_infos, &[signer_seeds])?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(owners: [Pubkey; 3])]
pub struct Initialize<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = MockMultisig::SPACE,
        seeds = [MULTISIG_SEED, creator.key().as_ref()],
        bump,
    )]
    pub multisig: Account<'info, MockMultisig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Execute<'info> {
    #[account(
        seeds = [MULTISIG_SEED, multisig.creator.as_ref()],
        bump = multisig.bump,
    )]
    pub multisig: Account<'info, MockMultisig>,
    pub signer_a: Signer<'info>,
    pub signer_b: Signer<'info>,
    /// CHECK: The test harness deliberately forwards to an arbitrary executable program.
    #[account(executable)]
    pub target_program: UncheckedAccount<'info>,
}

#[account]
#[derive(Debug, InitSpace)]
pub struct MockMultisig {
    pub creator: Pubkey,
    pub owners: [Pubkey; 3],
    pub threshold: u8,
    pub bump: u8,
}

impl MockMultisig {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

#[error_code]
pub enum MockMultisigError {
    #[msg("Mock multisig owners must be distinct and non-default")]
    InvalidOwners,
    #[msg("Mock multisig approvals must be distinct")]
    DuplicateApproval,
    #[msg("Mock multisig signer is not a member")]
    NotMember,
    #[msg("Mock multisig threshold is invalid")]
    InvalidThreshold,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_size_is_stable() {
        assert_eq!(MockMultisig::INIT_SPACE, 130);
        assert_eq!(MockMultisig::SPACE, 138);
    }
}
