use {
    anyhow::{bail, Context, Result},
    base64::{engine::general_purpose::STANDARD, Engine},
    borsh::{BorshDeserialize, BorshSerialize},
    serde::{Serialize, Serializer},
    sha2::{Digest, Sha256},
    std::fmt,
};

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pubkey(pub [u8; 32]);

impl fmt::Display for Pubkey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&bs58::encode(self.0).into_string())
    }
}

impl Serialize for Pubkey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

macro_rules! event_struct {
    ($name:ident { $($field:ident: $kind:ty),* $(,)? }) => {
        #[derive(BorshDeserialize, BorshSerialize, Clone, Debug, Serialize, PartialEq, Eq)]
        pub struct $name {
            pub version: u16,
            $(pub $field: $kind),*
        }
    };
}

event_struct!(ProtocolInitialized {
    protocol_config: Pubkey,
    admin: Pubkey,
    guardian: Pubkey,
});
event_struct!(ProtocolPauseUpdated {
    protocol_config: Pubkey,
    caller: Pubkey,
    previous_flags: u32,
    new_flags: u32,
});
event_struct!(EscrowCreated {
    escrow: Pubkey,
    escrow_token: Pubkey,
    maker: Pubkey,
    recipient: Pubkey,
    mint: Pubkey,
    amount: u64,
    created_at: i64,
    expires_at: i64,
    escrow_id: u64,
});
event_struct!(EscrowReleased {
    escrow: Pubkey,
    escrow_token: Pubkey,
    maker: Pubkey,
    recipient: Pubkey,
    recipient_token: Pubkey,
    mint: Pubkey,
    amount: u64,
});
event_struct!(EscrowRefunded {
    escrow: Pubkey,
    escrow_token: Pubkey,
    caller: Pubkey,
    maker: Pubkey,
    maker_destination: Pubkey,
    mint: Pubkey,
    amount: u64,
});
event_struct!(VaultCreated {
    vault: Pubkey,
    namespace_authority: Pubkey,
    authority: Pubkey,
    guardian: Pubkey,
    vault_id: u64,
});
event_struct!(VaultAuthorityUpdated {
    vault: Pubkey,
    previous_authority: Pubkey,
    new_authority: Pubkey,
});
event_struct!(VaultPauseUpdated {
    vault: Pubkey,
    caller: Pubkey,
    paused: bool,
});
event_struct!(VaultAssetRegistered {
    vault: Pubkey,
    vault_asset: Pubkey,
    vault_token: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
});
event_struct!(VaultDeposit {
    vault: Pubkey,
    vault_asset: Pubkey,
    vault_token: Pubkey,
    depositor: Pubkey,
    source_token: Pubkey,
    mint: Pubkey,
    amount: u64,
});
event_struct!(VaultWithdrawal {
    vault: Pubkey,
    vault_asset: Pubkey,
    vault_token: Pubkey,
    caller: Pubkey,
    destination_token: Pubkey,
    mint: Pubkey,
    amount: u64,
});
event_struct!(RoleGranted {
    vault: Pubkey,
    principal: Pubkey,
    permissions: u64,
});
event_struct!(RoleRevoked {
    vault: Pubkey,
    principal: Pubkey,
    previous_permissions: u64,
});
event_struct!(VaultLimitsUpdated {
    vault: Pubkey,
    vault_asset: Pubkey,
    mint: Pubkey,
    caller: Pubkey,
    max_per_transaction: u64,
    window_limit: u64,
    window_seconds: i64,
    timelock_threshold: u64,
    timelock_seconds: i64,
    request_execution_window_seconds: i64,
    window_started_at: i64,
    window_spent: u64,
});
event_struct!(WithdrawalRequested {
    vault: Pubkey,
    vault_asset: Pubkey,
    withdrawal_request: Pubkey,
    proposer: Pubkey,
    recipient_owner: Pubkey,
    recipient_token_account: Pubkey,
    mint: Pubkey,
    withdrawal_id: u64,
    amount: u64,
    created_at: i64,
    execute_after: i64,
    expires_at: i64,
});
event_struct!(WithdrawalExecuted {
    vault: Pubkey,
    vault_asset: Pubkey,
    withdrawal_request: Pubkey,
    caller: Pubkey,
    recipient_token_account: Pubkey,
    mint: Pubkey,
    withdrawal_id: u64,
    amount: u64,
});
event_struct!(WithdrawalCancelled {
    vault: Pubkey,
    withdrawal_request: Pubkey,
    caller: Pubkey,
    proposer: Pubkey,
    withdrawal_id: u64,
});

macro_rules! event_enum {
    ($($name:ident),+ $(,)?) => {
        #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
        #[serde(tag = "type", content = "data")]
        pub enum IronVaultEvent { $($name($name)),+ }

        impl IronVaultEvent {
            pub fn decode(bytes: &[u8]) -> Result<Self> {
                if bytes.len() < 10 {
                    bail!("event payload is shorter than discriminator and version");
                }
                $(if bytes[..8] == discriminator(stringify!($name)) {
                    return Ok(Self::$name($name::try_from_slice(&bytes[8..])
                        .with_context(|| format!("invalid {} payload", stringify!($name)))?));
                })+
                bail!("unknown IronVault event discriminator")
            }

            pub fn name(&self) -> &'static str {
                match self { $(Self::$name(_) => stringify!($name)),+ }
            }

            pub fn version(&self) -> u16 {
                match self { $(Self::$name(event) => event.version),+ }
            }

            pub fn payload(&self) -> serde_json::Value {
                serde_json::to_value(self).expect("serializable event")
            }
        }
    };
}

event_enum!(
    ProtocolInitialized,
    ProtocolPauseUpdated,
    EscrowCreated,
    EscrowReleased,
    EscrowRefunded,
    VaultCreated,
    VaultAuthorityUpdated,
    VaultPauseUpdated,
    VaultAssetRegistered,
    VaultDeposit,
    VaultWithdrawal,
    RoleGranted,
    RoleRevoked,
    VaultLimitsUpdated,
    WithdrawalRequested,
    WithdrawalExecuted,
    WithdrawalCancelled,
);

pub fn decode_program_data(log: &str) -> Result<Option<IronVaultEvent>> {
    let Some(encoded) = log.strip_prefix("Program data: ") else {
        return Ok(None);
    };
    let bytes = STANDARD
        .decode(encoded)
        .context("invalid base64 event log")?;
    IronVaultEvent::decode(&bytes).map(Some)
}

pub fn discriminator(name: &str) -> [u8; 8] {
    Sha256::digest(format!("event:{name}")).as_slice()[..8]
        .try_into()
        .expect("eight-byte slice")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_anchor_event_and_rejects_trailing_bytes() {
        let event = VaultPauseUpdated {
            version: 1,
            vault: Pubkey([1; 32]),
            caller: Pubkey([2; 32]),
            paused: true,
        };
        let mut encoded = discriminator("VaultPauseUpdated").to_vec();
        encoded.extend(borsh::to_vec(&event).unwrap());
        assert_eq!(
            IronVaultEvent::decode(&encoded).unwrap(),
            IronVaultEvent::VaultPauseUpdated(event)
        );
        encoded.push(0);
        assert!(IronVaultEvent::decode(&encoded).is_err());
    }

    #[test]
    fn program_data_parser_ignores_non_event_logs() {
        assert!(decode_program_data("Program log: hello").unwrap().is_none());
    }

    #[test]
    fn all_event_discriminators_are_unique() {
        let names = [
            "ProtocolInitialized",
            "ProtocolPauseUpdated",
            "EscrowCreated",
            "EscrowReleased",
            "EscrowRefunded",
            "VaultCreated",
            "VaultAuthorityUpdated",
            "VaultPauseUpdated",
            "VaultAssetRegistered",
            "VaultDeposit",
            "VaultWithdrawal",
            "RoleGranted",
            "RoleRevoked",
            "VaultLimitsUpdated",
            "WithdrawalRequested",
            "WithdrawalExecuted",
            "WithdrawalCancelled",
        ];
        let unique = names
            .iter()
            .map(|name| discriminator(name))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), names.len());
    }
}
