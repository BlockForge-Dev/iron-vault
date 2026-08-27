use anchor_lang::prelude::*;

#[event]
pub struct ProtocolInitialized {
    pub version: u16,
    pub protocol_config: Pubkey,
    pub admin: Pubkey,
    pub guardian: Pubkey,
}

#[event]
pub struct ProtocolPauseUpdated {
    pub version: u16,
    pub protocol_config: Pubkey,
    pub caller: Pubkey,
    pub previous_flags: u32,
    pub new_flags: u32,
}

/// Emitted after an escrow and its custody account are atomically funded.
#[event]
pub struct EscrowCreated {
    pub version: u16,
    pub escrow: Pubkey,
    pub escrow_token: Pubkey,
    pub maker: Pubkey,
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub created_at: i64,
    pub expires_at: i64,
    pub escrow_id: u64,
}

/// Emitted after a maker releases the exact escrow amount to its recipient.
#[event]
pub struct EscrowReleased {
    pub version: u16,
    pub escrow: Pubkey,
    pub escrow_token: Pubkey,
    pub maker: Pubkey,
    pub recipient: Pubkey,
    pub recipient_token: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted after an expired escrow is returned to its immutable maker.
#[event]
pub struct EscrowRefunded {
    pub version: u16,
    pub escrow: Pubkey,
    pub escrow_token: Pubkey,
    pub caller: Pubkey,
    pub maker: Pubkey,
    pub maker_destination: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted when an authority creates a new vault namespace.
#[event]
pub struct VaultCreated {
    pub version: u16,
    pub vault: Pubkey,
    pub namespace_authority: Pubkey,
    pub authority: Pubkey,
    pub guardian: Pubkey,
    pub vault_id: u64,
}

#[event]
pub struct VaultAuthorityUpdated {
    pub version: u16,
    pub vault: Pubkey,
    pub previous_authority: Pubkey,
    pub new_authority: Pubkey,
}

#[event]
pub struct VaultPauseUpdated {
    pub version: u16,
    pub vault: Pubkey,
    pub caller: Pubkey,
    pub paused: bool,
}

/// Emitted when a supported mint is registered with a vault.
#[event]
pub struct VaultAssetRegistered {
    pub version: u16,
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub vault_token: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
}

/// Emitted after an exact permissionless deposit reaches vault custody.
#[event]
pub struct VaultDeposit {
    pub version: u16,
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub vault_token: Pubkey,
    pub depositor: Pubkey,
    pub source_token: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted after the authority withdraws an exact amount from vault custody.
#[event]
pub struct VaultWithdrawal {
    pub version: u16,
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub vault_token: Pubkey,
    pub caller: Pubkey,
    pub destination_token: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

/// Emitted when the vault authority creates or replaces an exact role mask.
#[event]
pub struct RoleGranted {
    pub version: u16,
    pub vault: Pubkey,
    pub principal: Pubkey,
    pub permissions: u64,
}

/// Emitted when the vault authority immediately deactivates a role.
#[event]
pub struct RoleRevoked {
    pub version: u16,
    pub vault: Pubkey,
    pub principal: Pubkey,
    pub previous_permissions: u64,
}

/// Emitted after an authorized caller updates one asset's withdrawal limits.
#[event]
pub struct VaultLimitsUpdated {
    pub version: u16,
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub mint: Pubkey,
    pub caller: Pubkey,
    pub max_per_transaction: u64,
    pub window_limit: u64,
    pub window_seconds: i64,
    pub timelock_threshold: u64,
    pub timelock_seconds: i64,
    pub request_execution_window_seconds: i64,
    pub window_started_at: i64,
    pub window_spent: u64,
}

#[event]
pub struct WithdrawalRequested {
    pub version: u16,
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub withdrawal_request: Pubkey,
    pub proposer: Pubkey,
    pub recipient_owner: Pubkey,
    pub recipient_token_account: Pubkey,
    pub mint: Pubkey,
    pub withdrawal_id: u64,
    pub amount: u64,
    pub created_at: i64,
    pub execute_after: i64,
    pub expires_at: i64,
}

#[event]
pub struct WithdrawalExecuted {
    pub version: u16,
    pub vault: Pubkey,
    pub vault_asset: Pubkey,
    pub withdrawal_request: Pubkey,
    pub caller: Pubkey,
    pub recipient_token_account: Pubkey,
    pub mint: Pubkey,
    pub withdrawal_id: u64,
    pub amount: u64,
}

#[event]
pub struct WithdrawalCancelled {
    pub version: u16,
    pub vault: Pubkey,
    pub withdrawal_request: Pubkey,
    pub caller: Pubkey,
    pub proposer: Pubkey,
    pub withdrawal_id: u64,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        anchor_lang::{Discriminator, Event},
        std::collections::HashSet,
    };

    #[test]
    fn every_event_has_a_unique_anchor_discriminator() {
        let discriminators = [
            ProtocolInitialized::DISCRIMINATOR,
            ProtocolPauseUpdated::DISCRIMINATOR,
            EscrowCreated::DISCRIMINATOR,
            EscrowReleased::DISCRIMINATOR,
            EscrowRefunded::DISCRIMINATOR,
            VaultCreated::DISCRIMINATOR,
            VaultAuthorityUpdated::DISCRIMINATOR,
            VaultPauseUpdated::DISCRIMINATOR,
            VaultAssetRegistered::DISCRIMINATOR,
            VaultDeposit::DISCRIMINATOR,
            VaultWithdrawal::DISCRIMINATOR,
            RoleGranted::DISCRIMINATOR,
            RoleRevoked::DISCRIMINATOR,
            VaultLimitsUpdated::DISCRIMINATOR,
            WithdrawalRequested::DISCRIMINATOR,
            WithdrawalExecuted::DISCRIMINATOR,
            WithdrawalCancelled::DISCRIMINATOR,
        ];
        assert!(discriminators.iter().all(|value| value.len() == 8));
        assert_eq!(
            discriminators.iter().copied().collect::<HashSet<_>>().len(),
            discriminators.len()
        );
    }

    #[test]
    fn event_schema_version_is_nonzero() {
        assert!(crate::constants::INITIAL_SCHEMA_VERSION > 0);
        let event = ProtocolPauseUpdated {
            version: crate::constants::INITIAL_SCHEMA_VERSION,
            protocol_config: Pubkey::new_unique(),
            caller: Pubkey::new_unique(),
            previous_flags: 0,
            new_flags: 1,
        };
        let encoded = event.data();
        assert_eq!(&encoded[..8], ProtocolPauseUpdated::DISCRIMINATOR);
        assert_eq!(
            u16::from_le_bytes(encoded[8..10].try_into().unwrap()),
            crate::constants::INITIAL_SCHEMA_VERSION
        );
    }
}
