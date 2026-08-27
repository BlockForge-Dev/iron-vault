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
    }
}
