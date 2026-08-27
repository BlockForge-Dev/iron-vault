use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{program_option::COption, program_pack::Pack, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token::spl_token::{
        self,
        state::{Account as SplTokenAccount, AccountState, Mint as SplMint},
    },
    iron_vault::{
        state::{Vault, VaultAsset},
        ID,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const DECIMALS: u8 = 6;
const INITIAL_DEPOSITOR_BALANCE: u64 = 1_000_000;

struct Fixture {
    svm: LiteSVM,
    authority: Keypair,
    guardian: Keypair,
    depositor: Keypair,
    attacker: Keypair,
    mint: Pubkey,
    depositor_token: Pubkey,
    destination_token: Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let authority = Keypair::new();
        let guardian = Keypair::new();
        let depositor = Keypair::new();
        let attacker = Keypair::new();
        let mint = Pubkey::new_unique();
        let depositor_token = Pubkey::new_unique();
        let destination_token = Pubkey::new_unique();
        let mut svm = LiteSVM::new();

        svm.add_program(ID, include_bytes!("../../../target/deploy/iron_vault.so"))
            .unwrap();
        for signer in [&authority, &guardian, &depositor, &attacker] {
            svm.airdrop(&signer.pubkey(), 10_000_000_000).unwrap();
        }
        set_mint(
            &mut svm,
            mint,
            authority.pubkey(),
            INITIAL_DEPOSITOR_BALANCE,
        );
        set_token_account(
            &mut svm,
            depositor_token,
            mint,
            depositor.pubkey(),
            INITIAL_DEPOSITOR_BALANCE,
        );
        set_token_account(&mut svm, destination_token, mint, Pubkey::new_unique(), 0);

        Self {
            svm,
            authority,
            guardian,
            depositor,
            attacker,
            mint,
            depositor_token,
            destination_token,
        }
    }
}

fn set_mint(svm: &mut LiteSVM, address: Pubkey, authority: Pubkey, supply: u64) {
    let mint = SplMint {
        mint_authority: COption::Some(authority),
        supply,
        decimals: DECIMALS,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut data = vec![0; SplMint::LEN];
    SplMint::pack(mint, &mut data).unwrap();
    set_spl_account(svm, address, data);
}

fn set_token_account(svm: &mut LiteSVM, address: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) {
    let token = SplTokenAccount {
        mint,
        owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut data = vec![0; SplTokenAccount::LEN];
    SplTokenAccount::pack(token, &mut data).unwrap();
    set_spl_account(svm, address, data);
}

fn set_spl_account(svm: &mut LiteSVM, address: Pubkey, data: Vec<u8>) {
    let lamports = svm.minimum_balance_for_rent_exemption(data.len());
    svm.set_account(
        address,
        Account {
            lamports,
            data,
            owner: spl_token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn vault_address(namespace_authority: Pubkey, vault_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"vault",
            namespace_authority.as_ref(),
            &vault_id.to_le_bytes(),
        ],
        &ID,
    )
    .0
}

fn asset_addresses(vault: Pubkey, mint: Pubkey) -> (Pubkey, Pubkey) {
    let vault_asset =
        Pubkey::find_program_address(&[b"vault_asset", vault.as_ref(), mint.as_ref()], &ID).0;
    let vault_token =
        Pubkey::find_program_address(&[b"vault_token", vault.as_ref(), mint.as_ref()], &ID).0;
    (vault_asset, vault_token)
}

fn create_vault_instruction(
    authority: Pubkey,
    vault_id: u64,
    guardian: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(authority, vault_id);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::CreateVault { vault_id, guardian }.data(),
        iron_vault::accounts::CreateVault {
            authority,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn register_asset_instruction(
    fixture: &Fixture,
    authority: Pubkey,
    vault_id: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, fixture.mint);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::RegisterAsset {}.data(),
        iron_vault::accounts::RegisterAsset {
            authority,
            vault,
            mint: fixture.mint,
            vault_asset,
            vault_token,
            token_program: spl_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn deposit_instruction(
    fixture: &Fixture,
    vault_id: u64,
    depositor: Pubkey,
    source_token: Pubkey,
    amount: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, fixture.mint);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::Deposit { amount }.data(),
        iron_vault::accounts::Deposit {
            depositor,
            vault,
            mint: fixture.mint,
            vault_asset,
            source_token,
            vault_token,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    )
}

fn withdraw_instruction(
    fixture: &Fixture,
    vault_id: u64,
    authority: Pubkey,
    destination_token: Pubkey,
    amount: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, fixture.mint);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::Withdraw { amount }.data(),
        iron_vault::accounts::Withdraw {
            authority,
            vault,
            mint: fixture.mint,
            vault_asset,
            vault_token,
            destination_token,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    )
}

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instruction: anchor_lang::solana_program::instruction::Instruction,
) -> TransactionResult {
    svm.expire_blockhash();
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer]).unwrap();
    svm.send_transaction(transaction)
}

fn token_balance(svm: &LiteSVM, address: Pubkey) -> u64 {
    let account = svm.get_account(&address).unwrap();
    SplTokenAccount::unpack(&account.data).unwrap().amount
}

fn vault_state(svm: &LiteSVM, address: Pubkey) -> Vault {
    let account = svm.get_account(&address).unwrap();
    Vault::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn asset_state(svm: &LiteSVM, address: Pubkey) -> VaultAsset {
    let account = svm.get_account(&address).unwrap();
    VaultAsset::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn assert_error_message(result: TransactionResult, expected: &str) {
    let failure = result.expect_err("transaction unexpectedly succeeded");
    assert!(
        failure.meta.logs.iter().any(|line| line.contains(expected)),
        "expected error containing {expected:?}, got logs:\n{}",
        failure.meta.pretty_logs()
    );
}

fn registered_fixture(vault_id: u64) -> (Fixture, Pubkey, Pubkey, Pubkey) {
    let mut fixture = Fixture::new();
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, fixture.mint);
    let create = create_vault_instruction(
        fixture.authority.pubkey(),
        vault_id,
        fixture.guardian.pubkey(),
    );
    send(&mut fixture.svm, &fixture.authority, create).unwrap();
    let register = register_asset_instruction(&fixture, fixture.authority.pubkey(), vault_id);
    send(&mut fixture.svm, &fixture.authority, register).unwrap();
    (fixture, vault, vault_asset, vault_token)
}

fn funded_fixture(vault_id: u64, amount: u64) -> (Fixture, Pubkey, Pubkey, Pubkey) {
    let (mut fixture, vault, vault_asset, vault_token) = registered_fixture(vault_id);
    let deposit = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        amount,
    );
    send(&mut fixture.svm, &fixture.depositor, deposit).unwrap();
    (fixture, vault, vault_asset, vault_token)
}

#[test]
fn create_vault_succeeds() {
    let mut fixture = Fixture::new();
    let vault_id = 1;
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let instruction = create_vault_instruction(
        fixture.authority.pubkey(),
        vault_id,
        fixture.guardian.pubkey(),
    );
    let metadata = send(&mut fixture.svm, &fixture.authority, instruction).unwrap();

    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    let state = vault_state(&fixture.svm, vault);
    assert_eq!(state.namespace_authority, fixture.authority.pubkey());
    assert_eq!(state.authority, fixture.authority.pubkey());
    assert_eq!(state.guardian, fixture.guardian.pubkey());
    assert_eq!(state.vault_id, vault_id);
    assert_eq!(state.next_withdrawal_id, 0);
    assert!(!state.paused);
}

#[test]
fn invalid_guardian_rejected() {
    for vault_id in [2, 3] {
        let mut fixture = Fixture::new();
        let guardian = match vault_id {
            2 => Pubkey::default(),
            _ => fixture.authority.pubkey(),
        };
        let vault = vault_address(fixture.authority.pubkey(), vault_id);
        let instruction = create_vault_instruction(fixture.authority.pubkey(), vault_id, guardian);
        let result = send(&mut fixture.svm, &fixture.authority, instruction);
        assert_error_message(result, "Vault guardian is invalid");
        assert!(fixture.svm.get_account(&vault).is_none());
    }
}

#[test]
fn duplicate_vault_id_rejected() {
    let mut fixture = Fixture::new();
    let vault_id = 4;
    let instruction = create_vault_instruction(
        fixture.authority.pubkey(),
        vault_id,
        fixture.guardian.pubkey(),
    );
    send(&mut fixture.svm, &fixture.authority, instruction).unwrap();
    let duplicate = create_vault_instruction(
        fixture.authority.pubkey(),
        vault_id,
        fixture.guardian.pubkey(),
    );
    assert!(send(&mut fixture.svm, &fixture.authority, duplicate).is_err());
}

#[test]
fn register_asset_succeeds() {
    let vault_id = 5;
    let (fixture, vault, vault_asset, vault_token) = registered_fixture(vault_id);

    let asset = asset_state(&fixture.svm, vault_asset);
    assert_eq!(asset.vault, vault);
    assert_eq!(asset.mint, fixture.mint);
    assert_eq!(asset.token_program, spl_token::ID);
    assert!(asset.enabled);
    assert_eq!(asset.max_per_transaction, u64::MAX);
    assert_eq!(asset.window_limit, u64::MAX);
    assert_eq!(asset.timelock_threshold, u64::MAX);
    let custody = fixture.svm.get_account(&vault_token).unwrap();
    let custody = SplTokenAccount::unpack(&custody.data).unwrap();
    assert_eq!(custody.owner, vault);
    assert_eq!(custody.mint, fixture.mint);
    assert_eq!(custody.amount, 0);
}

#[test]
fn non_authority_cannot_register_asset() {
    let mut fixture = Fixture::new();
    let vault_id = 6;
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, fixture.mint);
    let create = create_vault_instruction(
        fixture.authority.pubkey(),
        vault_id,
        fixture.guardian.pubkey(),
    );
    send(&mut fixture.svm, &fixture.authority, create).unwrap();
    let register = register_asset_instruction(&fixture, fixture.attacker.pubkey(), vault_id);
    let result = send(&mut fixture.svm, &fixture.attacker, register);

    assert_error_message(result, "Caller is not the vault authority");
    assert!(fixture.svm.get_account(&vault_asset).is_none());
    assert!(fixture.svm.get_account(&vault_token).is_none());
}

#[test]
fn duplicate_asset_registration_rejected() {
    let vault_id = 7;
    let (mut fixture, _, vault_asset, vault_token) = registered_fixture(vault_id);
    let duplicate = register_asset_instruction(&fixture, fixture.authority.pubkey(), vault_id);
    assert!(send(&mut fixture.svm, &fixture.authority, duplicate).is_err());
    assert!(fixture.svm.get_account(&vault_asset).is_some());
    assert_eq!(token_balance(&fixture.svm, vault_token), 0);
}

#[test]
fn permissionless_deposit_succeeds() {
    let vault_id = 8;
    let amount = 400_000;
    let (mut fixture, _, _, vault_token) = registered_fixture(vault_id);
    let instruction = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        amount,
    );
    let metadata = send(&mut fixture.svm, &fixture.depositor, instruction).unwrap();

    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.depositor_token),
        INITIAL_DEPOSITOR_BALANCE - amount
    );
}

#[test]
fn zero_deposit_rejected() {
    let vault_id = 9;
    let (mut fixture, _, _, vault_token) = registered_fixture(vault_id);
    let instruction = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        0,
    );
    let result = send(&mut fixture.svm, &fixture.depositor, instruction);

    assert_error_message(result, "Vault transfer amount must be greater than zero");
    assert_eq!(token_balance(&fixture.svm, vault_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.depositor_token),
        INITIAL_DEPOSITOR_BALANCE
    );
}

#[test]
fn wrong_deposit_source_owner_rejected() {
    let vault_id = 10;
    let (mut fixture, _, _, vault_token) = registered_fixture(vault_id);
    let foreign_token = Pubkey::new_unique();
    set_token_account(
        &mut fixture.svm,
        foreign_token,
        fixture.mint,
        fixture.authority.pubkey(),
        10,
    );
    let instruction = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        foreign_token,
        10,
    );
    let result = send(&mut fixture.svm, &fixture.depositor, instruction);

    assert_error_message(result, "Depositor does not own the source token account");
    assert_eq!(token_balance(&fixture.svm, vault_token), 0);
    assert_eq!(token_balance(&fixture.svm, foreign_token), 10);
}

#[test]
fn wrong_deposit_source_mint_rejected() {
    let vault_id = 11;
    let (mut fixture, _, _, vault_token) = registered_fixture(vault_id);
    let wrong_mint = Pubkey::new_unique();
    let wrong_token = Pubkey::new_unique();
    set_mint(&mut fixture.svm, wrong_mint, fixture.depositor.pubkey(), 10);
    set_token_account(
        &mut fixture.svm,
        wrong_token,
        wrong_mint,
        fixture.depositor.pubkey(),
        10,
    );
    let instruction = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        wrong_token,
        10,
    );
    let result = send(&mut fixture.svm, &fixture.depositor, instruction);

    assert_error_message(result, "Deposit source mint does not match the vault asset");
    assert_eq!(token_balance(&fixture.svm, vault_token), 0);
    assert_eq!(token_balance(&fixture.svm, wrong_token), 10);
}

#[test]
fn authority_withdraws_exact_amount() {
    let vault_id = 12;
    let deposit_amount = 500_000;
    let withdraw_amount = 125_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, deposit_amount);
    let instruction = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        withdraw_amount,
    );
    let metadata = send(&mut fixture.svm, &fixture.authority, instruction).unwrap();

    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    assert_eq!(
        token_balance(&fixture.svm, vault_token),
        deposit_amount - withdraw_amount
    );
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        withdraw_amount
    );
}

#[test]
fn guardian_cannot_withdraw() {
    let vault_id = 13;
    let amount = 100_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    let instruction = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.guardian.pubkey(),
        fixture.destination_token,
        amount,
    );
    let result = send(&mut fixture.svm, &fixture.guardian, instruction);

    assert_error_message(result, "Caller is not the vault authority");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn random_wallet_cannot_withdraw() {
    let vault_id = 14;
    let amount = 100_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    let instruction = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.attacker.pubkey(),
        fixture.destination_token,
        amount,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, instruction);

    assert_error_message(result, "Caller is not the vault authority");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn wrong_withdrawal_destination_mint_rejected() {
    let vault_id = 15;
    let amount = 100_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    let wrong_mint = Pubkey::new_unique();
    let wrong_token = Pubkey::new_unique();
    set_mint(&mut fixture.svm, wrong_mint, fixture.authority.pubkey(), 0);
    set_token_account(
        &mut fixture.svm,
        wrong_token,
        wrong_mint,
        fixture.attacker.pubkey(),
        0,
    );
    let instruction = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        wrong_token,
        amount,
    );
    let result = send(&mut fixture.svm, &fixture.authority, instruction);

    assert_error_message(
        result,
        "Withdrawal destination mint does not match the vault asset",
    );
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, wrong_token), 0);
}

#[test]
fn insufficient_vault_funds_rejected() {
    let vault_id = 16;
    let amount = 100_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    let instruction = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        amount + 1,
    );
    let result = send(&mut fixture.svm, &fixture.authority, instruction);

    assert_error_message(result, "Vault custody has insufficient funds");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn zero_withdrawal_rejected() {
    let vault_id = 17;
    let amount = 100_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    let instruction = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        0,
    );
    let result = send(&mut fixture.svm, &fixture.authority, instruction);

    assert_error_message(result, "Vault transfer amount must be greater than zero");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn cross_vault_asset_substitution_rejected() {
    let first_vault_id = 18;
    let second_vault_id = 19;
    let amount = 100_000;
    let (mut fixture, _, _, first_vault_token) = funded_fixture(first_vault_id, amount);

    let create_second = create_vault_instruction(
        fixture.authority.pubkey(),
        second_vault_id,
        fixture.guardian.pubkey(),
    );
    send(&mut fixture.svm, &fixture.authority, create_second).unwrap();
    let register_second =
        register_asset_instruction(&fixture, fixture.authority.pubkey(), second_vault_id);
    send(&mut fixture.svm, &fixture.authority, register_second).unwrap();
    let second_vault = vault_address(fixture.authority.pubkey(), second_vault_id);
    let (second_vault_asset, _) = asset_addresses(second_vault, fixture.mint);

    let mut withdrawal = withdraw_instruction(
        &fixture,
        first_vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        amount,
    );
    withdrawal.accounts[3].pubkey = second_vault_asset;
    let result = send(&mut fixture.svm, &fixture.authority, withdrawal);

    assert!(result.is_err());
    assert_eq!(token_balance(&fixture.svm, first_vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}
