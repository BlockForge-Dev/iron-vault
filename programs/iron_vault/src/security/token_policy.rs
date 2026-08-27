use {
    anchor_lang::prelude::*,
    anchor_spl::token_interface::spl_token_2022::{
        extension::{BaseStateWithExtensions, StateWithExtensions},
        state::Mint,
    },
};

/// Returns whether a mint satisfies IronVault's fail-closed v1 token policy.
///
/// Legacy SPL mints have no extension area and are accepted. Token-2022 mints
/// are accepted only when their TLV extension list is empty. This deliberately
/// rejects every initialized extension, including unknown future extensions,
/// until its semantics and required CPI accounts have been reviewed.
pub fn mint_extensions_supported(mint: &AccountInfo<'_>) -> Result<bool> {
    if *mint.owner == anchor_spl::token::ID {
        return Ok(true);
    }

    let data = mint.try_borrow_data()?;
    let state = StateWithExtensions::<Mint>::unpack(&data)?;
    Ok(state.get_extension_types()?.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::{program_option::COption, program_pack::Pack};
    use anchor_spl::token_interface::spl_token_2022::extension::{
        permanent_delegate::PermanentDelegate, BaseStateWithExtensionsMut, ExtensionType,
        StateWithExtensionsMut,
    };

    fn account_info<'a>(owner: &'a Pubkey, data: &'a mut [u8]) -> AccountInfo<'a> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(1));
        AccountInfo::new(key, false, false, lamports, data, owner, false)
    }

    #[test]
    fn legacy_and_vanilla_token_2022_are_supported() {
        let mint = Mint {
            mint_authority: COption::None,
            supply: 0,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        };

        let mut legacy_data = vec![0; Mint::LEN];
        Mint::pack(mint, &mut legacy_data).unwrap();
        let legacy = account_info(&anchor_spl::token::ID, &mut legacy_data);
        assert!(mint_extensions_supported(&legacy).unwrap());

        let mut token_2022_data = vec![0; Mint::LEN];
        Mint::pack(mint, &mut token_2022_data).unwrap();
        let token_2022 = account_info(
            &anchor_spl::token_interface::spl_token_2022::ID,
            &mut token_2022_data,
        );
        assert!(mint_extensions_supported(&token_2022).unwrap());
    }

    #[test]
    fn any_initialized_extension_is_rejected() {
        let length =
            ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::PermanentDelegate])
                .unwrap();
        let mut data = vec![0; length];
        let mut state = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut data).unwrap();
        state.init_extension::<PermanentDelegate>(true).unwrap();
        state.base = Mint {
            mint_authority: COption::None,
            supply: 0,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        };
        state.pack_base();
        state.init_account_type().unwrap();

        let info = account_info(&anchor_spl::token_interface::spl_token_2022::ID, &mut data);
        assert!(!mint_extensions_supported(&info).unwrap());
    }
}
