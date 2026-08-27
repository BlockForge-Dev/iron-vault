/// Schema version reserved for the first protocol-state implementation.
pub const INITIAL_SCHEMA_VERSION: u16 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_schema_version_is_nonzero() {
        assert_ne!(INITIAL_SCHEMA_VERSION, 0);
    }
}
