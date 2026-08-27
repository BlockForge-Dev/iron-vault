use {
    crate::{
        constants::ROLE_SEED,
        error::IronVaultError,
        state::{RoleAssignment, Vault},
    },
    anchor_lang::prelude::*,
};

pub fn validate_role_permission(
    role_info: &AccountInfo<'_>,
    vault: &Account<'_, Vault>,
    caller: Pubkey,
    permission: u64,
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
    require!(role.has(permission), IronVaultError::MissingVaultPermission);

    Ok(())
}
