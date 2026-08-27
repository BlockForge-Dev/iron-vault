use {
    crate::{
        constants::{
            PAUSE_VAULT_CONFIG, PERMISSION_MANAGE_LIMITS, PROTOCOL_SEED, VAULT_ASSET_SEED,
            VAULT_SEED,
        },
        error::IronVaultError,
        events::VaultLimitsUpdated,
        security::pause::require_protocol_active,
        security::permissions::validate_role_permission,
        security::token_policy::mint_extensions_supported,
        state::{ProtocolConfig, Vault, VaultAsset},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct UpdateLimits<'info> {
    pub caller: Signer<'info>,
    #[account(seeds = [PROTOCOL_SEED], bump = protocol_config.bump)]
    pub protocol_config: Account<'info, ProtocolConfig>,
    #[account(
        seeds = [
            VAULT_SEED,
            vault.namespace_authority.as_ref(),
            vault.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        constraint = mint.to_account_info().owner == &vault_asset.token_program
            @ IronVaultError::InvalidTokenProgram,
        constraint = mint_extensions_supported(&mint.to_account_info())?
            @ IronVaultError::UnsupportedTokenExtension,
    )]
    pub mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,
    #[account(
        mut,
        seeds = [VAULT_ASSET_SEED, vault.key().as_ref(), mint.key().as_ref()],
        bump = vault_asset.bump,
        has_one = vault,
        has_one = mint,
    )]
    pub vault_asset: Account<'info, VaultAsset>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn update(
    ctx: Context<UpdateLimits>,
    max_per_transaction: u64,
    window_limit: u64,
    window_seconds: i64,
    timelock_threshold: u64,
    timelock_seconds: i64,
    request_execution_window_seconds: i64,
) -> Result<()> {
    require_protocol_active(&ctx.accounts.protocol_config, PAUSE_VAULT_CONFIG)?;
    authorize(&ctx)?;
    require!(
        max_per_transaction > 0
            && window_limit > 0
            && window_seconds > 0
            && max_per_transaction <= window_limit
            && timelock_threshold <= max_per_transaction
            && timelock_seconds > 0
            && request_execution_window_seconds > 0,
        IronVaultError::InvalidWithdrawalPolicy
    );

    let asset = &mut ctx.accounts.vault_asset;
    let now = ctx.accounts.clock.unix_timestamp;
    if asset.window_seconds == 0 {
        asset.window_started_at = now;
        asset.window_spent = 0;
    } else if asset.window_seconds != window_seconds {
        let current_window_ends = asset
            .window_started_at
            .checked_add(asset.window_seconds)
            .ok_or(IronVaultError::WithdrawalPolicyOverflow)?;
        require_gte!(
            now,
            current_window_ends,
            IronVaultError::LiveWindowDurationChange
        );
        asset.window_started_at = now;
        asset.window_spent = 0;
    }

    asset.max_per_transaction = max_per_transaction;
    asset.window_limit = window_limit;
    asset.window_seconds = window_seconds;
    asset.timelock_threshold = timelock_threshold;
    asset.timelock_seconds = timelock_seconds;
    asset.request_execution_window_seconds = request_execution_window_seconds;

    emit!(VaultLimitsUpdated {
        version: crate::constants::INITIAL_SCHEMA_VERSION,
        vault: ctx.accounts.vault.key(),
        vault_asset: asset.key(),
        mint: asset.mint,
        caller: ctx.accounts.caller.key(),
        max_per_transaction,
        window_limit,
        window_seconds,
        timelock_threshold,
        timelock_seconds,
        request_execution_window_seconds,
        window_started_at: asset.window_started_at,
        window_spent: asset.window_spent,
    });

    Ok(())
}

fn authorize(ctx: &Context<UpdateLimits>) -> Result<()> {
    if ctx.accounts.caller.key() == ctx.accounts.vault.authority {
        require!(
            ctx.remaining_accounts.is_empty(),
            IronVaultError::UnexpectedLimitAccounts
        );
    } else {
        require!(
            ctx.remaining_accounts.len() <= 1,
            IronVaultError::UnexpectedLimitAccounts
        );
        require_eq!(
            ctx.remaining_accounts.len(),
            1,
            IronVaultError::MissingVaultPermission
        );
        validate_role_permission(
            &ctx.remaining_accounts[0],
            &ctx.accounts.vault,
            ctx.accounts.caller.key(),
            PERMISSION_MANAGE_LIMITS,
        )?;
    }
    Ok(())
}
