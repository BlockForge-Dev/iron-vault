use {
    crate::{
        constants::{ESCROW_SEED, ESCROW_TOKEN_SEED, PAUSE_ESCROW_CREATE, PROTOCOL_SEED},
        error::IronVaultError,
        events::EscrowCreated,
        security::{pause::require_protocol_active, token_policy::mint_extensions_supported},
        state::{Escrow, EscrowStatus, ProtocolConfig},
    },
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

#[derive(Accounts)]
#[instruction(escrow_id: u64)]
pub struct CreateEscrow<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(seeds = [PROTOCOL_SEED], bump = protocol_config.bump)]
    pub protocol_config: Account<'info, ProtocolConfig>,
    #[account(
        constraint = mint.to_account_info().owner == token_program.to_account_info().key
            @ IronVaultError::InvalidTokenProgram,
        constraint = mint_extensions_supported(&mint.to_account_info())?
            @ IronVaultError::UnsupportedTokenExtension,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        constraint = maker_token.owner == maker.key() @ IronVaultError::InvalidSourceOwner,
        constraint = maker_token.mint == mint.key() @ IronVaultError::InvalidSourceMint,
        constraint = maker_token.to_account_info().owner == token_program.to_account_info().key
            @ IronVaultError::InvalidTokenProgram,
    )]
    pub maker_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        space = Escrow::SPACE,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        init,
        payer = maker,
        seeds = [ESCROW_TOKEN_SEED, escrow.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = escrow,
        token::token_program = token_program,
    )]
    pub escrow_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn create(
    ctx: Context<CreateEscrow>,
    escrow_id: u64,
    recipient: Pubkey,
    amount: u64,
    expires_at: i64,
) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_ESCROW_CREATE)?;
    let maker = ctx.accounts.maker.key();
    let now = Clock::get()?.unix_timestamp;

    require_gt!(amount, 0, IronVaultError::InvalidAmount);
    require!(
        recipient != Pubkey::default() && recipient != maker,
        IronVaultError::InvalidRecipient
    );
    require_gt!(expires_at, now, IronVaultError::InvalidExpiry);
    require_gte!(
        ctx.accounts.maker_token.amount,
        amount,
        IronVaultError::InsufficientFunds
    );
    require_eq!(
        ctx.accounts.escrow_token.amount,
        0,
        IronVaultError::InvalidCustodyBalance
    );

    let escrow = &mut ctx.accounts.escrow;
    escrow.maker = maker;
    escrow.recipient = recipient;
    escrow.mint = ctx.accounts.mint.key();
    escrow.token_program = ctx.accounts.token_program.key();
    escrow.escrow_id = escrow_id;
    escrow.amount = amount;
    escrow.created_at = now;
    escrow.expires_at = expires_at;
    escrow.status = EscrowStatus::Funded;
    escrow.bump = ctx.bumps.escrow;
    escrow.reserved = [0; 30];

    let source_before = ctx.accounts.maker_token.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.maker_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.escrow_token.to_account_info(),
                authority: ctx.accounts.maker.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.maker_token.reload()?;
    ctx.accounts.escrow_token.reload()?;
    require_eq!(
        ctx.accounts.maker_token.amount,
        source_before - amount,
        IronVaultError::InvalidSourceBalance
    );
    require_eq!(
        ctx.accounts.escrow_token.amount,
        amount,
        IronVaultError::InvalidCustodyBalance
    );

    emit!(EscrowCreated {
        version: crate::constants::INITIAL_SCHEMA_VERSION,
        escrow: ctx.accounts.escrow.key(),
        escrow_token: ctx.accounts.escrow_token.key(),
        maker,
        recipient,
        mint: ctx.accounts.mint.key(),
        amount,
        created_at: now,
        expires_at,
        escrow_id,
    });

    Ok(())
}
