/// Schema version reserved for the first protocol-state implementation.
pub const INITIAL_SCHEMA_VERSION: u16 = 1;

/// Namespace for escrow state PDAs.
pub const ESCROW_SEED: &[u8] = b"escrow";

/// Namespace for escrow custody token account PDAs.
pub const ESCROW_TOKEN_SEED: &[u8] = b"escrow_token";

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
}
