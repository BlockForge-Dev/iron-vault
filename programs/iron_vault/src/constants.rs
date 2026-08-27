/// Schema version reserved for the first protocol-state implementation.
pub const INITIAL_SCHEMA_VERSION: u16 = 1;

/// Namespace for escrow state PDAs.
pub const ESCROW_SEED: &[u8] = b"escrow";

/// Namespace for escrow custody token account PDAs.
pub const ESCROW_TOKEN_SEED: &[u8] = b"escrow_token";

/// Namespace for vault state PDAs.
pub const VAULT_SEED: &[u8] = b"vault";

/// Namespace for per-mint vault asset state PDAs.
pub const VAULT_ASSET_SEED: &[u8] = b"vault_asset";

/// Namespace for per-mint vault custody token account PDAs.
pub const VAULT_TOKEN_SEED: &[u8] = b"vault_token";

/// Namespace for per-principal role assignment PDAs.
pub const ROLE_SEED: &[u8] = b"role";

/// Namespace for immutable timelocked withdrawal request PDAs.
pub const WITHDRAWAL_SEED: &[u8] = b"withdrawal";

pub const PERMISSION_WITHDRAW: u64 = 1 << 0;
pub const PERMISSION_REQUEST_WITHDRAWAL: u64 = 1 << 1;
pub const PERMISSION_CANCEL_WITHDRAWAL: u64 = 1 << 2;
pub const PERMISSION_MANAGE_ASSETS: u64 = 1 << 3;
pub const PERMISSION_MANAGE_LIMITS: u64 = 1 << 4;
pub const PERMISSION_MANAGE_ROLES: u64 = 1 << 5;
pub const KNOWN_PERMISSIONS: u64 = PERMISSION_WITHDRAW
    | PERMISSION_REQUEST_WITHDRAWAL
    | PERMISSION_CANCEL_WITHDRAWAL
    | PERMISSION_MANAGE_ASSETS
    | PERMISSION_MANAGE_LIMITS
    | PERMISSION_MANAGE_ROLES;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_schema_version_is_nonzero() {
        assert_ne!(INITIAL_SCHEMA_VERSION, 0);
    }

    #[test]
    fn escrow_namespaces_match_the_account_model() {
        assert_eq!(ESCROW_SEED, b"escrow");
        assert_eq!(ESCROW_TOKEN_SEED, b"escrow_token");
    }

    #[test]
    fn vault_namespaces_match_the_account_model() {
        assert_eq!(VAULT_SEED, b"vault");
        assert_eq!(VAULT_ASSET_SEED, b"vault_asset");
        assert_eq!(VAULT_TOKEN_SEED, b"vault_token");
        assert_eq!(ROLE_SEED, b"role");
        assert_eq!(WITHDRAWAL_SEED, b"withdrawal");
    }

    #[test]
    fn permission_mask_contains_exactly_the_reserved_bits() {
        assert_eq!(KNOWN_PERMISSIONS, 0b11_1111);
        assert_eq!(PERMISSION_WITHDRAW, 1);
        assert_eq!(PERMISSION_REQUEST_WITHDRAWAL, 2);
        assert_eq!(PERMISSION_CANCEL_WITHDRAWAL, 4);
        assert_eq!(PERMISSION_MANAGE_ASSETS, 8);
        assert_eq!(PERMISSION_MANAGE_LIMITS, 16);
        assert_eq!(PERMISSION_MANAGE_ROLES, 32);
    }
}
