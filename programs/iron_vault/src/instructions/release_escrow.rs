use {
    crate::{
        constants::{ESCROW_SEED, ESCROW_TOKEN_SEED, PAUSE_ESCROW_RELEASE, PROTOCOL_SEED},
        error::IronVaultError,
        events::EscrowReleased,
        security::{pause::require_protocol_active, token_policy::mint_extensions_supported},
        state::{Escrow, EscrowStatus, ProtocolConfig},
    },
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

#[derive(Accounts)]
pub struct ReleaseEscrow<'info> {
    pub maker: Signer<'info>,
    #[account(seeds = [PROTOCOL_SEED], bump = protocol_config.bump)]
    pub protocol_config: Account<'info, ProtocolConfig>,
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
        constraint = recipient_token.owner == escrow.recipient @ IronVaultError::InvalidRecipientOwner,
        constraint = recipient_token.mint == escrow.mint @ IronVaultError::InvalidRecipientMint,
        constraint = recipient_token.to_account_info().owner == token_program.to_account_info().key
            @ IronVaultError::InvalidTokenProgram,
    )]
    pub recipient_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn release(ctx: Context<ReleaseEscrow>) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_ESCROW_RELEASE)?;
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

    token_interface::transfer_checked(
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
