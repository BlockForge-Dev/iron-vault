use {
    crate::{
        constants::{
            PERMISSION_WITHDRAW, ROLE_SEED, VAULT_ASSET_SEED, VAULT_SEED, VAULT_TOKEN_SEED,
        },
        error::IronVaultError,
        events::VaultWithdrawal,
        state::{RoleAssignment, Vault, VaultAsset},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked},
};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [VAULT_ASSET_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump = vault_asset.bump,
        has_one = vault,
        has_one = mint,
        constraint = vault_asset.token_program == token_program.key(),
    )]
    pub vault_asset: Account<'info, VaultAsset>,
    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump,
        constraint = vault_token.owner == vault.key() @ IronVaultError::InvalidVaultCustodyBalance,
        constraint = vault_token.mint == vault_asset.mint @ IronVaultError::InvalidWithdrawalDestinationMint,
    )]
    pub vault_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = destination_token.mint == vault_asset.mint @ IronVaultError::InvalidWithdrawalDestinationMint,
    )]
    pub destination_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn withdraw_tokens(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    if ctx.accounts.caller.key() == ctx.accounts.vault.authority {
        require!(
            ctx.remaining_accounts.is_empty(),
            IronVaultError::UnexpectedWithdrawalAccounts
        );
    } else {
        require!(
            ctx.remaining_accounts.len() <= 1,
            IronVaultError::UnexpectedWithdrawalAccounts
        );
        require_eq!(
            ctx.remaining_accounts.len(),
            1,
            IronVaultError::MissingVaultPermission
        );
        validate_withdraw_role(
            &ctx.remaining_accounts[0],
            &ctx.accounts.vault,
            ctx.accounts.caller.key(),
        )?;
    }
    require_gt!(amount, 0, IronVaultError::InvalidVaultAmount);
    require!(!ctx.accounts.vault.paused, IronVaultError::VaultPaused);
    require!(
        ctx.accounts.vault_asset.enabled,
        IronVaultError::VaultAssetDisabled
    );
    require_gte!(
        ctx.accounts.vault_token.amount,
        amount,
        IronVaultError::InsufficientVaultFunds
    );

    let custody_before = ctx.accounts.vault_token.amount;
    let destination_before = ctx.accounts.destination_token.amount;
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(IronVaultError::InvalidWithdrawalDestinationBalance)?;
    let vault_id = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = [ctx.accounts.vault.bump];
    let signer_seeds: &[&[u8]] = &[
        VAULT_SEED,
        ctx.accounts.vault.namespace_authority.as_ref(),
        &vault_id,
        &bump,
    ];
    let signer = &[signer_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.destination_token.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.vault_token.reload()?;
    ctx.accounts.destination_token.reload()?;
    require_eq!(
        ctx.accounts.vault_token.amount,
        custody_before - amount,
        IronVaultError::InvalidVaultCustodyBalance
    );
    require_eq!(
        ctx.accounts.destination_token.amount,
        destination_after,
        IronVaultError::InvalidWithdrawalDestinationBalance
    );

    emit!(VaultWithdrawal {
        vault: ctx.accounts.vault.key(),
        vault_asset: ctx.accounts.vault_asset.key(),
        vault_token: ctx.accounts.vault_token.key(),
        caller: ctx.accounts.caller.key(),
        destination_token: ctx.accounts.destination_token.key(),
        mint: ctx.accounts.mint.key(),
        amount,
    });

    Ok(())
}

fn validate_withdraw_role(
    role_info: &AccountInfo<'_>,
    vault: &Account<'_, Vault>,
    caller: Pubkey,
) -> Result<()> {
    let (expected_role, _) = Pubkey::find_program_address(
        &[ROLE_SEED, vault.key().as_ref(), caller.as_ref()],
        &crate::ID,
    );
    require_keys_eq!(
        *role_info.key,
        expected_role,
        IronVaultError::MissingVaultPermission
    );
    require_keys_eq!(
        *role_info.owner,
        crate::ID,
        IronVaultError::MissingVaultPermission
    );

    let data = role_info.try_borrow_data()?;
    let role = RoleAssignment::try_deserialize(&mut data.as_ref())
        .map_err(|_| error!(IronVaultError::MissingVaultPermission))?;
    require_keys_eq!(
        role.vault,
        vault.key(),
        IronVaultError::MissingVaultPermission
    );
    require_keys_eq!(
        role.principal,
        caller,
        IronVaultError::MissingVaultPermission
    );
    require!(
        role.has(PERMISSION_WITHDRAW),
        IronVaultError::MissingVaultPermission
    );

    Ok(())
}
