use {
    crate::{
        constants::{ESCROW_SEED, ESCROW_TOKEN_SEED},
        error::IronVaultError,
        events::EscrowReleased,
        state::{Escrow, EscrowStatus},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked},
};

#[derive(Accounts)]
pub struct ReleaseEscrow<'info> {
    pub maker: Signer<'info>,
    #[account(
        mut,
        seeds = [
            ESCROW_SEED,
            escrow.maker.as_ref(),
            escrow.escrow_id.to_le_bytes().as_ref(),
        ],
        bump = escrow.bump,
        has_one = maker,
        has_one = mint,
        constraint = escrow.token_program == token_program.key(),
    )]
    pub escrow: Account<'info, Escrow>,
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [ESCROW_TOKEN_SEED, escrow.key().as_ref()],
        bump,
        constraint = escrow_token.owner == escrow.key() @ IronVaultError::InvalidCustodyBalance,
        constraint = escrow_token.mint == escrow.mint @ IronVaultError::InvalidSourceMint,
    )]
    pub escrow_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = recipient_token.owner == escrow.recipient @ IronVaultError::InvalidRecipientOwner,
        constraint = recipient_token.mint == escrow.mint @ IronVaultError::InvalidRecipientMint,
    )]
    pub recipient_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn release(ctx: Context<ReleaseEscrow>) -> Result<()> {
    let escrow = &ctx.accounts.escrow;
    require!(
        escrow.status == EscrowStatus::Funded,
        IronVaultError::EscrowNotFunded
    );
    require_gt!(
        escrow.expires_at,
        Clock::get()?.unix_timestamp,
        IronVaultError::EscrowExpired
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
    let recipient_before = ctx.accounts.recipient_token.amount;

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.escrow_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.recipient_token.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.escrow_token.reload()?;
    ctx.accounts.recipient_token.reload()?;
    require_eq!(
        ctx.accounts.escrow_token.amount,
        custody_before - amount,
        IronVaultError::InvalidCustodyBalance
    );
    require_eq!(
        ctx.accounts.recipient_token.amount,
        recipient_before
            .checked_add(amount)
            .ok_or(IronVaultError::InvalidRecipientBalance)?,
        IronVaultError::InvalidRecipientBalance
    );

    ctx.accounts.escrow.status = EscrowStatus::Released;
    emit!(EscrowReleased {
        escrow: ctx.accounts.escrow.key(),
        escrow_token: ctx.accounts.escrow_token.key(),
        maker: ctx.accounts.escrow.maker,
        recipient: ctx.accounts.escrow.recipient,
        recipient_token: ctx.accounts.recipient_token.key(),
        mint: ctx.accounts.escrow.mint,
        amount,
    });

    Ok(())
}
