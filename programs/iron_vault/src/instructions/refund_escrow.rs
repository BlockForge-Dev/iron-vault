use {
    crate::{
        constants::{ESCROW_SEED, ESCROW_TOKEN_SEED},
        error::IronVaultError,
        events::EscrowRefunded,
        security::token_policy::mint_extensions_supported,
        state::{Escrow, EscrowStatus},
    },
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

#[derive(Accounts)]
pub struct RefundEscrow<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [
            ESCROW_SEED,
            escrow.maker.as_ref(),
            escrow.escrow_id.to_le_bytes().as_ref(),
        ],
        bump = escrow.bump,
        has_one = mint,
        constraint = escrow.token_program == token_program.key(),
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        constraint = mint.to_account_info().owner == token_program.to_account_info().key
            @ IronVaultError::InvalidTokenProgram,
        constraint = mint_extensions_supported(&mint.to_account_info())?
            @ IronVaultError::UnsupportedTokenExtension,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        seeds = [ESCROW_TOKEN_SEED, escrow.key().as_ref()],
        bump,
        constraint = escrow_token.owner == escrow.key() @ IronVaultError::InvalidCustodyBalance,
        constraint = escrow_token.mint == escrow.mint @ IronVaultError::InvalidSourceMint,
        constraint = escrow_token.to_account_info().owner == token_program.to_account_info().key
            @ IronVaultError::InvalidTokenProgram,
    )]
    pub escrow_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        constraint = maker_destination.owner == escrow.maker @ IronVaultError::InvalidMakerDestinationOwner,
        constraint = maker_destination.mint == escrow.mint @ IronVaultError::InvalidMakerDestinationMint,
        constraint = maker_destination.to_account_info().owner == token_program.to_account_info().key
            @ IronVaultError::InvalidTokenProgram,
    )]
    pub maker_destination: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn refund(ctx: Context<RefundEscrow>) -> Result<()> {
    let escrow = &ctx.accounts.escrow;
    require!(
        escrow.status == EscrowStatus::Funded,
        IronVaultError::EscrowNotFunded
    );
    require_gte!(
        ctx.accounts.clock.unix_timestamp,
        escrow.expires_at,
        IronVaultError::EscrowNotExpired
    );
    require_gte!(
        ctx.accounts.escrow_token.amount,
        escrow.amount,
        IronVaultError::InsufficientFunds
    );

    let amount = escrow.amount;
    let escrow_id = escrow.escrow_id.to_le_bytes();
    let bump = [escrow.bump];
    let signer_seeds: &[&[u8]] = &[ESCROW_SEED, escrow.maker.as_ref(), &escrow_id, &bump];
    let signer = &[signer_seeds];
    let custody_before = ctx.accounts.escrow_token.amount;
    let destination_before = ctx.accounts.maker_destination.amount;

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.escrow_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.maker_destination.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.escrow_token.reload()?;
    ctx.accounts.maker_destination.reload()?;
    require_eq!(
        ctx.accounts.escrow_token.amount,
        custody_before - amount,
        IronVaultError::InvalidCustodyBalance
    );
    require_eq!(
        ctx.accounts.maker_destination.amount,
        destination_before
            .checked_add(amount)
            .ok_or(IronVaultError::InvalidMakerDestinationBalance)?,
        IronVaultError::InvalidMakerDestinationBalance
    );

    ctx.accounts.escrow.status = EscrowStatus::Refunded;
    emit!(EscrowRefunded {
        escrow: ctx.accounts.escrow.key(),
        escrow_token: ctx.accounts.escrow_token.key(),
        caller: ctx.accounts.caller.key(),
        maker: ctx.accounts.escrow.maker,
        maker_destination: ctx.accounts.maker_destination.key(),
        mint: ctx.accounts.escrow.mint,
        amount,
    });

    Ok(())
}
