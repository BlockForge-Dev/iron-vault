use {crate::events::IronVaultEvent, tracing::warn};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alert {
    ProtocolPaused {
        flags: u32,
    },
    VaultPaused {
        vault: String,
    },
    LargeWithdrawalRequested {
        request: String,
        amount: u64,
    },
    VaultAuthorityChanged {
        vault: String,
        new_authority: String,
    },
    GuardianChanged {
        scope: String,
        new_guardian: String,
    },
    ProgramUpgradeObserved,
    ObserverFallingBehind {
        lag: u64,
    },
}

pub fn classify_event(event: &IronVaultEvent, large_withdrawal_threshold: u64) -> Option<Alert> {
    match event {
        IronVaultEvent::ProtocolPauseUpdated(event) if event.new_flags != 0 => {
            Some(Alert::ProtocolPaused {
                flags: event.new_flags,
            })
        }
        IronVaultEvent::VaultPauseUpdated(event) if event.paused => Some(Alert::VaultPaused {
            vault: event.vault.to_string(),
        }),
        IronVaultEvent::WithdrawalRequested(event)
            if event.amount >= large_withdrawal_threshold =>
        {
            Some(Alert::LargeWithdrawalRequested {
                request: event.withdrawal_request.to_string(),
                amount: event.amount,
            })
        }
        IronVaultEvent::VaultAuthorityUpdated(event) => Some(Alert::VaultAuthorityChanged {
            vault: event.vault.to_string(),
            new_authority: event.new_authority.to_string(),
        }),
        _ => None,
    }
}

pub fn emit(alert: Alert) {
    warn!(alert = ?alert, "IronVault observer alert");
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::events::{Pubkey, WithdrawalRequested},
    };

    #[test]
    fn large_withdrawal_threshold_is_inclusive() {
        let event = IronVaultEvent::WithdrawalRequested(WithdrawalRequested {
            version: 1,
            vault: Pubkey([1; 32]),
            vault_asset: Pubkey([2; 32]),
            withdrawal_request: Pubkey([3; 32]),
            proposer: Pubkey([4; 32]),
            recipient_owner: Pubkey([5; 32]),
            recipient_token_account: Pubkey([6; 32]),
            mint: Pubkey([7; 32]),
            withdrawal_id: 9,
            amount: 50_000,
            created_at: 10,
            execute_after: 20,
            expires_at: 30,
        });
        assert!(matches!(
            classify_event(&event, 50_000),
            Some(Alert::LargeWithdrawalRequested { .. })
        ));
    }
}
