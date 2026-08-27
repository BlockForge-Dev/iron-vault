use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{
            instruction::AccountMeta, program_option::COption, program_pack::Pack, system_program,
            sysvar::SysvarId,
        },
        AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token::spl_token::{
        self,
        state::{Account as SplTokenAccount, AccountState, Mint as SplMint},
    },
    iron_vault::{
        constants::{
            PAUSE_VAULT_CONFIG, PAUSE_VAULT_OUTFLOW, PERMISSION_MANAGE_LIMITS,
            PERMISSION_REQUEST_WITHDRAWAL, PERMISSION_WITHDRAW,
        },
        state::{
            ProtocolConfig, RoleAssignment, Vault, VaultAsset, WithdrawalRequest, WithdrawalStatus,
        },
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
        let initialize = initialize_protocol_instruction(
            authority.pubkey(),
            authority.pubkey(),
            guardian.pubkey(),
        );
        send(&mut svm, &authority, initialize).unwrap();
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

fn protocol_address() -> Pubkey {
    Pubkey::find_program_address(&[b"protocol"], &ID).0
}

fn initialize_protocol_instruction(
    initializer: Pubkey,
    admin: Pubkey,
    guardian: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::InitializeProtocol { admin, guardian }.data(),
        iron_vault::accounts::InitializeProtocol {
            initializer,
            protocol_config: protocol_address(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn set_protocol_pause_instruction(
    caller: Pubkey,
    flags: u32,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::SetProtocolPause { flags }.data(),
        iron_vault::accounts::SetProtocolPause {
            caller,
            protocol_config: protocol_address(),
        }
        .to_account_metas(None),
    )
}

fn pause_vault_instruction(
    fixture: &Fixture,
    vault_id: u64,
    caller: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::PauseVault {}.data(),
        iron_vault::accounts::PauseVault {
            caller,
            vault: vault_address(fixture.authority.pubkey(), vault_id),
        }
        .to_account_metas(None),
    )
}

fn unpause_vault_instruction(
    fixture: &Fixture,
    vault_id: u64,
    authority: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::UnpauseVault {}.data(),
        iron_vault::accounts::UnpauseVault {
            authority,
            vault: vault_address(fixture.authority.pubkey(), vault_id),
        }
        .to_account_metas(None),
    )
}

fn asset_addresses(vault: Pubkey, mint: Pubkey) -> (Pubkey, Pubkey) {
    let vault_asset =
        Pubkey::find_program_address(&[b"vault_asset", vault.as_ref(), mint.as_ref()], &ID).0;
    let vault_token =
        Pubkey::find_program_address(&[b"vault_token", vault.as_ref(), mint.as_ref()], &ID).0;
    (vault_asset, vault_token)
}

fn role_address(vault: Pubkey, principal: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"role", vault.as_ref(), principal.as_ref()], &ID).0
}

fn withdrawal_address(vault: Pubkey, withdrawal_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[b"withdrawal", vault.as_ref(), &withdrawal_id.to_le_bytes()],
        &ID,
    )
    .0
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
            protocol_config: protocol_address(),
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
    register_asset_instruction_for_mint(fixture, authority, vault_id, fixture.mint)
}

fn register_asset_instruction_for_mint(
    fixture: &Fixture,
    authority: Pubkey,
    vault_id: u64,
    mint: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, mint);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::RegisterAsset {}.data(),
        iron_vault::accounts::RegisterAsset {
            authority,
            protocol_config: protocol_address(),
            vault,
            mint,
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
    deposit_instruction_for_mint(
        fixture,
        vault_id,
        depositor,
        fixture.mint,
        source_token,
        amount,
    )
}

fn deposit_instruction_for_mint(
    fixture: &Fixture,
    vault_id: u64,
    depositor: Pubkey,
    mint: Pubkey,
    source_token: Pubkey,
    amount: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, mint);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::Deposit { amount }.data(),
        iron_vault::accounts::Deposit {
            depositor,
            vault,
            mint,
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
    withdraw_instruction_with_role(
        fixture,
        vault_id,
        authority,
        destination_token,
        amount,
        None,
    )
}

fn withdraw_instruction_with_role(
    fixture: &Fixture,
    vault_id: u64,
    caller: Pubkey,
    destination_token: Pubkey,
    amount: u64,
    role_assignment: Option<Pubkey>,
) -> anchor_lang::solana_program::instruction::Instruction {
    withdraw_instruction_with_role_for_mint(
        fixture,
        vault_id,
        caller,
        fixture.mint,
        destination_token,
        amount,
        role_assignment,
    )
}

fn withdraw_instruction_with_role_for_mint(
    fixture: &Fixture,
    vault_id: u64,
    caller: Pubkey,
    mint: Pubkey,
    destination_token: Pubkey,
    amount: u64,
    role_assignment: Option<Pubkey>,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, mint);
    let mut instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::Withdraw { amount }.data(),
        iron_vault::accounts::Withdraw {
            caller,
            protocol_config: protocol_address(),
            vault,
            mint,
            vault_asset,
            vault_token,
            destination_token,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    );
    if let Some(role_assignment) = role_assignment {
        instruction
            .accounts
            .push(AccountMeta::new_readonly(role_assignment, false));
    }
    instruction
}

fn grant_role_instruction(
    fixture: &Fixture,
    vault_id: u64,
    principal: Pubkey,
    permissions: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let role_assignment = role_address(vault, principal);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::GrantRole {
            principal,
            permissions,
        }
        .data(),
        iron_vault::accounts::GrantRole {
            authority: fixture.authority.pubkey(),
            protocol_config: protocol_address(),
            vault,
            role_assignment,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn revoke_role_instruction(
    fixture: &Fixture,
    vault_id: u64,
    principal: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let role_assignment = role_address(vault, principal);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::RevokeRole { principal }.data(),
        iron_vault::accounts::RevokeRole {
            authority: fixture.authority.pubkey(),
            protocol_config: protocol_address(),
            vault,
            role_assignment,
        }
        .to_account_metas(None),
    )
}

fn update_limits_instruction(
    fixture: &Fixture,
    vault_id: u64,
    caller: Pubkey,
    mint: Pubkey,
    max_per_transaction: u64,
    window_limit: u64,
    window_seconds: i64,
    role_assignment: Option<Pubkey>,
) -> anchor_lang::solana_program::instruction::Instruction {
    update_full_policy_instruction(
        fixture,
        vault_id,
        caller,
        mint,
        max_per_transaction,
        window_limit,
        window_seconds,
        max_per_transaction,
        3_600,
        3_600,
        role_assignment,
    )
}

#[allow(clippy::too_many_arguments)]
fn update_full_policy_instruction(
    fixture: &Fixture,
    vault_id: u64,
    caller: Pubkey,
    mint: Pubkey,
    max_per_transaction: u64,
    window_limit: u64,
    window_seconds: i64,
    timelock_threshold: u64,
    timelock_seconds: i64,
    request_execution_window_seconds: i64,
    role_assignment: Option<Pubkey>,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, _) = asset_addresses(vault, mint);
    let mut instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::UpdateLimits {
            max_per_transaction,
            window_limit,
            window_seconds,
            timelock_threshold,
            timelock_seconds,
            request_execution_window_seconds,
        }
        .data(),
        iron_vault::accounts::UpdateLimits {
            caller,
            protocol_config: protocol_address(),
            vault,
            mint,
            vault_asset,
            clock: Clock::id(),
        }
        .to_account_metas(None),
    );
    if let Some(role_assignment) = role_assignment {
        instruction
            .accounts
            .push(AccountMeta::new_readonly(role_assignment, false));
    }
    instruction
}

fn request_withdrawal_instruction(
    fixture: &Fixture,
    vault_id: u64,
    proposer: Pubkey,
    recipient_token: Pubkey,
    withdrawal_id: u64,
    amount: u64,
    role_assignment: Option<Pubkey>,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, _) = asset_addresses(vault, fixture.mint);
    let withdrawal_request = withdrawal_address(vault, withdrawal_id);
    let mut instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::RequestWithdrawal { amount }.data(),
        iron_vault::accounts::RequestWithdrawal {
            proposer,
            protocol_config: protocol_address(),
            vault,
            mint: fixture.mint,
            vault_asset,
            recipient_token,
            withdrawal_request,
            system_program: system_program::ID,
            clock: Clock::id(),
        }
        .to_account_metas(None),
    );
    if let Some(role_assignment) = role_assignment {
        instruction
            .accounts
            .push(AccountMeta::new_readonly(role_assignment, false));
    }
    instruction
}

fn execute_withdrawal_instruction(
    fixture: &Fixture,
    vault_id: u64,
    withdrawal_id: u64,
    caller: Pubkey,
    mint: Pubkey,
    recipient_token: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let (vault_asset, vault_token) = asset_addresses(vault, mint);
    let withdrawal_request = withdrawal_address(vault, withdrawal_id);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::ExecuteWithdrawal {}.data(),
        iron_vault::accounts::ExecuteWithdrawal {
            caller,
            protocol_config: protocol_address(),
            vault,
            mint,
            vault_asset,
            withdrawal_request,
            vault_token,
            recipient_token,
            token_program: spl_token::ID,
            clock: Clock::id(),
        }
        .to_account_metas(None),
    )
}

fn cancel_withdrawal_instruction(
    fixture: &Fixture,
    vault_id: u64,
    withdrawal_id: u64,
    caller: Pubkey,
    role_assignment: Option<Pubkey>,
) -> anchor_lang::solana_program::instruction::Instruction {
    let vault = vault_address(fixture.authority.pubkey(), vault_id);
    let withdrawal_request = withdrawal_address(vault, withdrawal_id);
    let mut instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::CancelWithdrawal {}.data(),
        iron_vault::accounts::CancelWithdrawal {
            caller,
            vault,
            withdrawal_request,
        }
        .to_account_metas(None),
    );
    if let Some(role_assignment) = role_assignment {
        instruction
            .accounts
            .push(AccountMeta::new_readonly(role_assignment, false));
    }
    instruction
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

fn protocol_state(svm: &LiteSVM) -> ProtocolConfig {
    let account = svm.get_account(&protocol_address()).unwrap();
    ProtocolConfig::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn asset_state(svm: &LiteSVM, address: Pubkey) -> VaultAsset {
    let account = svm.get_account(&address).unwrap();
    VaultAsset::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn set_asset_state(svm: &mut LiteSVM, address: Pubkey, state: &VaultAsset) {
    let mut account = svm.get_account(&address).unwrap();
    let mut data = Vec::with_capacity(VaultAsset::SPACE);
    state.try_serialize(&mut data).unwrap();
    assert_eq!(data.len(), VaultAsset::SPACE);
    account.data = data;
    svm.set_account(address, account).unwrap();
}

fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock: Clock = svm.get_sysvar();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
}

fn role_state(svm: &LiteSVM, address: Pubkey) -> RoleAssignment {
    let account = svm.get_account(&address).unwrap();
    RoleAssignment::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn withdrawal_state(svm: &LiteSVM, address: Pubkey) -> WithdrawalRequest {
    let account = svm.get_account(&address).unwrap();
    WithdrawalRequest::try_deserialize(&mut account.data.as_slice()).unwrap()
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

fn configure_limits(
    fixture: &mut Fixture,
    vault_id: u64,
    mint: Pubkey,
    max_per_transaction: u64,
    window_limit: u64,
    window_seconds: i64,
) {
    let instruction = update_limits_instruction(
        fixture,
        vault_id,
        fixture.authority.pubkey(),
        mint,
        max_per_transaction,
        window_limit,
        window_seconds,
        None,
    );
    send(&mut fixture.svm, &fixture.authority, instruction).unwrap();
}

fn configure_timelock_policy(
    fixture: &mut Fixture,
    vault_id: u64,
    timelock_threshold: u64,
    timelock_seconds: i64,
) {
    let instruction = update_full_policy_instruction(
        fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.mint,
        INITIAL_DEPOSITOR_BALANCE,
        INITIAL_DEPOSITOR_BALANCE,
        86_400,
        timelock_threshold,
        timelock_seconds,
        3_600,
        None,
    );
    send(&mut fixture.svm, &fixture.authority, instruction).unwrap();
}

fn requested_fixture(
    vault_id: u64,
    amount: u64,
    timelock_seconds: i64,
) -> (Fixture, Pubkey, Pubkey, Pubkey, Pubkey) {
    let (mut fixture, vault, vault_asset, vault_token) =
        funded_fixture(vault_id, amount.saturating_mul(2));
    configure_timelock_policy(&mut fixture, vault_id, 5_000, timelock_seconds);
    let request = withdrawal_address(vault, 0);
    let instruction = request_withdrawal_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        0,
        amount,
        None,
    );
    send(&mut fixture.svm, &fixture.authority, instruction).unwrap();
    (fixture, vault, vault_asset, vault_token, request)
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

    assert_error_message(result, "Caller lacks the required vault permission");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn random_user_rejected() {
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

    assert_error_message(result, "Caller lacks the required vault permission");
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

#[test]
fn authority_can_manage_roles() {
    let vault_id = 20;
    let (mut fixture, vault, _, _) = registered_fixture(vault_id);
    let principal = fixture.attacker.pubkey();
    let role = role_address(vault, principal);
    let initial_permissions = PERMISSION_WITHDRAW | PERMISSION_REQUEST_WITHDRAWAL;
    let grant = grant_role_instruction(&fixture, vault_id, principal, initial_permissions);
    let metadata = send(&mut fixture.svm, &fixture.authority, grant).unwrap();

    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    let state = role_state(&fixture.svm, role);
    assert_eq!(state.vault, vault);
    assert_eq!(state.principal, principal);
    assert_eq!(state.permissions, initial_permissions);
    assert!(state.active);

    let replacement =
        grant_role_instruction(&fixture, vault_id, principal, PERMISSION_REQUEST_WITHDRAWAL);
    send(&mut fixture.svm, &fixture.authority, replacement).unwrap();
    let state = role_state(&fixture.svm, role);
    assert_eq!(state.permissions, PERMISSION_REQUEST_WITHDRAWAL);
    assert!(state.active);
}

#[test]
fn operator_can_withdraw_when_authorized() {
    let vault_id = 21;
    let amount = 100_000;
    let (mut fixture, vault, _, vault_token) = funded_fixture(vault_id, amount);
    let operator = fixture.attacker.pubkey();
    let role = role_address(vault, operator);
    let grant = grant_role_instruction(&fixture, vault_id, operator, PERMISSION_WITHDRAW);
    send(&mut fixture.svm, &fixture.authority, grant).unwrap();

    let withdrawal = withdraw_instruction_with_role(
        &fixture,
        vault_id,
        operator,
        fixture.destination_token,
        amount,
        Some(role),
    );
    send(&mut fixture.svm, &fixture.attacker, withdrawal).unwrap();

    assert_eq!(token_balance(&fixture.svm, vault_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount
    );
}

#[test]
fn operator_without_permission_rejected() {
    let vault_id = 22;
    let amount = 100_000;
    let (mut fixture, vault, _, vault_token) = funded_fixture(vault_id, amount);
    let operator = fixture.attacker.pubkey();
    let role = role_address(vault, operator);
    let grant = grant_role_instruction(&fixture, vault_id, operator, PERMISSION_REQUEST_WITHDRAWAL);
    send(&mut fixture.svm, &fixture.authority, grant).unwrap();

    let withdrawal = withdraw_instruction_with_role(
        &fixture,
        vault_id,
        operator,
        fixture.destination_token,
        amount,
        Some(role),
    );
    let result = send(&mut fixture.svm, &fixture.attacker, withdrawal);

    assert_error_message(result, "Caller lacks the required vault permission");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn revoked_operator_immediately_rejected() {
    let vault_id = 23;
    let amount = 100_000;
    let (mut fixture, vault, _, vault_token) = funded_fixture(vault_id, amount);
    let operator = fixture.attacker.pubkey();
    let role = role_address(vault, operator);
    let grant = grant_role_instruction(&fixture, vault_id, operator, PERMISSION_WITHDRAW);
    send(&mut fixture.svm, &fixture.authority, grant).unwrap();
    let revoke = revoke_role_instruction(&fixture, vault_id, operator);
    send(&mut fixture.svm, &fixture.authority, revoke).unwrap();
    let state = role_state(&fixture.svm, role);
    assert!(!state.active);
    assert_eq!(state.permissions, 0);

    let withdrawal = withdraw_instruction_with_role(
        &fixture,
        vault_id,
        operator,
        fixture.destination_token,
        amount,
        Some(role),
    );
    let result = send(&mut fixture.svm, &fixture.attacker, withdrawal);

    assert_error_message(result, "Caller lacks the required vault permission");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn role_for_other_vault_cannot_be_reused() {
    let first_vault_id = 24;
    let second_vault_id = 25;
    let amount = 100_000;
    let (mut fixture, first_vault, _, first_vault_token) = funded_fixture(first_vault_id, amount);
    let operator = fixture.attacker.pubkey();
    let first_role = role_address(first_vault, operator);
    let grant = grant_role_instruction(&fixture, first_vault_id, operator, PERMISSION_WITHDRAW);
    send(&mut fixture.svm, &fixture.authority, grant).unwrap();

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
    let (_, second_vault_token) = asset_addresses(second_vault, fixture.mint);
    let deposit_second = deposit_instruction(
        &fixture,
        second_vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        amount,
    );
    send(&mut fixture.svm, &fixture.depositor, deposit_second).unwrap();

    let withdrawal = withdraw_instruction_with_role(
        &fixture,
        second_vault_id,
        operator,
        fixture.destination_token,
        amount,
        Some(first_role),
    );
    let result = send(&mut fixture.svm, &fixture.attacker, withdrawal);

    assert!(result.is_err());
    assert_eq!(token_balance(&fixture.svm, first_vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, second_vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn invalid_role_grants_rejected() {
    for (vault_id, permissions) in [(26, 0), (27, 1_u64 << 63)] {
        let (mut fixture, vault, _, _) = registered_fixture(vault_id);
        let principal = fixture.attacker.pubkey();
        let role = role_address(vault, principal);
        let grant = grant_role_instruction(&fixture, vault_id, principal, permissions);
        let result = send(&mut fixture.svm, &fixture.authority, grant);
        assert_error_message(result, "Role permission mask is invalid");
        assert!(fixture.svm.get_account(&role).is_none());
    }

    for (vault_id, principal_kind) in [(28, 0), (29, 1), (30, 2)] {
        let (mut fixture, vault, _, _) = registered_fixture(vault_id);
        let principal = match principal_kind {
            0 => Pubkey::default(),
            1 => fixture.authority.pubkey(),
            _ => fixture.guardian.pubkey(),
        };
        let role = role_address(vault, principal);
        let grant = grant_role_instruction(&fixture, vault_id, principal, PERMISSION_WITHDRAW);
        let result = send(&mut fixture.svm, &fixture.authority, grant);
        assert_error_message(result, "Role principal is invalid");
        assert!(fixture.svm.get_account(&role).is_none());
    }
}

#[test]
fn non_authority_cannot_manage_roles() {
    let vault_id = 31;
    let (mut fixture, vault, _, _) = registered_fixture(vault_id);
    let principal = fixture.depositor.pubkey();
    let role = role_address(vault, principal);
    let mut grant = grant_role_instruction(&fixture, vault_id, principal, PERMISSION_WITHDRAW);
    grant.accounts[0].pubkey = fixture.attacker.pubkey();
    let result = send(&mut fixture.svm, &fixture.attacker, grant);

    assert_error_message(result, "Caller is not the vault authority");
    assert!(fixture.svm.get_account(&role).is_none());
}

#[test]
fn authority_withdraw_rejects_unexpected_accounts() {
    let vault_id = 32;
    let amount = 100_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    let withdrawal = withdraw_instruction_with_role(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        amount,
        Some(system_program::ID),
    );
    let result = send(&mut fixture.svm, &fixture.authority, withdrawal);

    assert_error_message(result, "Unexpected withdrawal accounts");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn withdraw_inside_limit_succeeds() {
    let vault_id = 40;
    let deposit_amount = 60_000;
    let amount = 7_000;
    let (mut fixture, _, vault_asset, vault_token) = funded_fixture(vault_id, deposit_amount);
    let mint = fixture.mint;
    configure_limits(&mut fixture, vault_id, mint, 10_000, 50_000, 86_400);
    let withdrawal = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        amount,
    );
    send(&mut fixture.svm, &fixture.authority, withdrawal).unwrap();

    assert_eq!(
        token_balance(&fixture.svm, vault_token),
        deposit_amount - amount
    );
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount
    );
    assert_eq!(asset_state(&fixture.svm, vault_asset).window_spent, amount);
}

#[test]
fn per_tx_limit_enforced() {
    let vault_id = 41;
    let deposit_amount = 20_000;
    let (mut fixture, _, vault_asset, vault_token) = funded_fixture(vault_id, deposit_amount);
    let mint = fixture.mint;
    configure_limits(&mut fixture, vault_id, mint, 10_000, 50_000, 86_400);
    let withdrawal = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        10_001,
    );
    let result = send(&mut fixture.svm, &fixture.authority, withdrawal);

    assert_error_message(result, "Per-transaction withdrawal limit exceeded");
    assert_eq!(token_balance(&fixture.svm, vault_token), deposit_amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
    assert_eq!(asset_state(&fixture.svm, vault_asset).window_spent, 0);
}

#[test]
fn window_limit_enforced() {
    let vault_id = 42;
    let deposit_amount = 60_000;
    let (mut fixture, _, vault_asset, vault_token) = funded_fixture(vault_id, deposit_amount);
    let mint = fixture.mint;
    configure_limits(&mut fixture, vault_id, mint, 10_000, 50_000, 86_400);

    for amount in [10_000, 10_000, 10_000, 10_000, 8_000] {
        let withdrawal = withdraw_instruction(
            &fixture,
            vault_id,
            fixture.authority.pubkey(),
            fixture.destination_token,
            amount,
        );
        send(&mut fixture.svm, &fixture.authority, withdrawal).unwrap();
    }
    assert_eq!(asset_state(&fixture.svm, vault_asset).window_spent, 48_000);

    let rejected = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        7_000,
    );
    let result = send(&mut fixture.svm, &fixture.authority, rejected);
    assert_error_message(result, "Rolling-window withdrawal limit exceeded");
    assert_eq!(token_balance(&fixture.svm, vault_token), 12_000);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        48_000
    );
    assert_eq!(asset_state(&fixture.svm, vault_asset).window_spent, 48_000);
}

#[test]
fn window_rolls_over() {
    let vault_id = 43;
    let deposit_amount = 30_000;
    let (mut fixture, _, vault_asset, vault_token) = funded_fixture(vault_id, deposit_amount);
    let mint = fixture.mint;
    configure_limits(&mut fixture, vault_id, mint, 10_000, 10_000, 100);
    let first = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        8_000,
    );
    send(&mut fixture.svm, &fixture.authority, first).unwrap();
    let first_window = asset_state(&fixture.svm, vault_asset);
    let rollover_at = first_window.window_started_at + first_window.window_seconds;
    set_clock(&mut fixture.svm, rollover_at);

    let second = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        7_000,
    );
    send(&mut fixture.svm, &fixture.authority, second).unwrap();
    let rolled = asset_state(&fixture.svm, vault_asset);
    assert_eq!(rolled.window_started_at, rollover_at);
    assert_eq!(rolled.window_spent, 7_000);
    assert_eq!(token_balance(&fixture.svm, vault_token), 15_000);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        15_000
    );
}

#[test]
fn overflow_cannot_bypass_limit() {
    let vault_id = 44;
    let amount = 1;
    let (mut fixture, _, vault_asset, vault_token) = funded_fixture(vault_id, amount);
    let mint = fixture.mint;
    configure_limits(&mut fixture, vault_id, mint, u64::MAX, u64::MAX, 1_000);
    let mut corrupted = asset_state(&fixture.svm, vault_asset);
    corrupted.window_spent = u64::MAX;
    set_asset_state(&mut fixture.svm, vault_asset, &corrupted);

    let withdrawal = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        amount,
    );
    let result = send(&mut fixture.svm, &fixture.authority, withdrawal);

    assert_error_message(result, "Withdrawal policy arithmetic overflow");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
    assert_eq!(
        asset_state(&fixture.svm, vault_asset).window_spent,
        u64::MAX
    );
}

#[test]
fn different_assets_have_independent_limits() {
    let vault_id = 45;
    let first_amount = 20_000;
    let second_amount = 30_000;
    let (mut fixture, vault, first_asset, first_vault_token) =
        funded_fixture(vault_id, first_amount);
    let first_mint = fixture.mint;
    let second_mint = Pubkey::new_unique();
    let second_source = Pubkey::new_unique();
    let second_destination = Pubkey::new_unique();
    set_mint(
        &mut fixture.svm,
        second_mint,
        fixture.depositor.pubkey(),
        second_amount,
    );
    set_token_account(
        &mut fixture.svm,
        second_source,
        second_mint,
        fixture.depositor.pubkey(),
        second_amount,
    );
    set_token_account(
        &mut fixture.svm,
        second_destination,
        second_mint,
        fixture.attacker.pubkey(),
        0,
    );
    let register_second = register_asset_instruction_for_mint(
        &fixture,
        fixture.authority.pubkey(),
        vault_id,
        second_mint,
    );
    send(&mut fixture.svm, &fixture.authority, register_second).unwrap();
    let (second_asset, second_vault_token) = asset_addresses(vault, second_mint);
    let deposit_second = deposit_instruction_for_mint(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        second_mint,
        second_source,
        second_amount,
    );
    send(&mut fixture.svm, &fixture.depositor, deposit_second).unwrap();
    configure_limits(&mut fixture, vault_id, first_mint, 10_000, 10_000, 100);
    configure_limits(&mut fixture, vault_id, second_mint, 20_000, 20_000, 100);

    let first_withdrawal = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        10_000,
    );
    send(&mut fixture.svm, &fixture.authority, first_withdrawal).unwrap();
    let first_rejected = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        1,
    );
    assert_error_message(
        send(&mut fixture.svm, &fixture.authority, first_rejected),
        "Rolling-window withdrawal limit exceeded",
    );
    let second_withdrawal = withdraw_instruction_with_role_for_mint(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        second_mint,
        second_destination,
        15_000,
        None,
    );
    send(&mut fixture.svm, &fixture.authority, second_withdrawal).unwrap();

    assert_eq!(asset_state(&fixture.svm, first_asset).window_spent, 10_000);
    assert_eq!(asset_state(&fixture.svm, second_asset).window_spent, 15_000);
    assert_eq!(token_balance(&fixture.svm, first_vault_token), 10_000);
    assert_eq!(token_balance(&fixture.svm, second_vault_token), 15_000);
    assert_eq!(token_balance(&fixture.svm, second_destination), 15_000);
}

#[test]
fn different_vaults_have_independent_limits() {
    let first_vault_id = 46;
    let second_vault_id = 47;
    let amount = 10_000;
    let (mut fixture, _, first_asset, first_vault_token) = funded_fixture(first_vault_id, amount);
    let mint = fixture.mint;
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
    let (second_asset, second_vault_token) = asset_addresses(second_vault, mint);
    let deposit_second = deposit_instruction(
        &fixture,
        second_vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        amount,
    );
    send(&mut fixture.svm, &fixture.depositor, deposit_second).unwrap();
    configure_limits(&mut fixture, first_vault_id, mint, amount, amount, 100);
    configure_limits(&mut fixture, second_vault_id, mint, amount, amount, 100);

    for vault_id in [first_vault_id, second_vault_id] {
        let withdrawal = withdraw_instruction(
            &fixture,
            vault_id,
            fixture.authority.pubkey(),
            fixture.destination_token,
            amount,
        );
        send(&mut fixture.svm, &fixture.authority, withdrawal).unwrap();
    }

    assert_eq!(asset_state(&fixture.svm, first_asset).window_spent, amount);
    assert_eq!(asset_state(&fixture.svm, second_asset).window_spent, amount);
    assert_eq!(token_balance(&fixture.svm, first_vault_token), 0);
    assert_eq!(token_balance(&fixture.svm, second_vault_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount * 2
    );
}

#[test]
fn operator_with_manage_limits_can_update_policy() {
    let vault_id = 48;
    let (mut fixture, vault, vault_asset, _) = registered_fixture(vault_id);
    let operator = fixture.attacker.pubkey();
    let role = role_address(vault, operator);
    let grant = grant_role_instruction(&fixture, vault_id, operator, PERMISSION_MANAGE_LIMITS);
    send(&mut fixture.svm, &fixture.authority, grant).unwrap();
    let update = update_limits_instruction(
        &fixture,
        vault_id,
        operator,
        fixture.mint,
        10_000,
        50_000,
        86_400,
        Some(role),
    );
    send(&mut fixture.svm, &fixture.attacker, update).unwrap();

    let state = asset_state(&fixture.svm, vault_asset);
    assert_eq!(state.max_per_transaction, 10_000);
    assert_eq!(state.window_limit, 50_000);
    assert_eq!(state.window_seconds, 86_400);
}

#[test]
fn invalid_and_live_duration_policy_updates_rejected() {
    let vault_id = 49;
    let (mut fixture, _, vault_asset, _) = registered_fixture(vault_id);
    for (max_per_transaction, window_limit, window_seconds) in
        [(0, 10, 10), (11, 10, 10), (1, 0, 10), (1, 10, 0)]
    {
        let update = update_limits_instruction(
            &fixture,
            vault_id,
            fixture.authority.pubkey(),
            fixture.mint,
            max_per_transaction,
            window_limit,
            window_seconds,
            None,
        );
        let result = send(&mut fixture.svm, &fixture.authority, update);
        assert_error_message(result, "Withdrawal policy is invalid");
    }

    let mint = fixture.mint;
    configure_limits(&mut fixture, vault_id, mint, 10, 100, 100);
    let duration_change = update_limits_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.mint,
        10,
        100,
        200,
        None,
    );
    let result = send(&mut fixture.svm, &fixture.authority, duration_change);
    assert_error_message(
        result,
        "Cannot change duration while the withdrawal window is live",
    );
    let state = asset_state(&fixture.svm, vault_asset);
    assert_eq!(state.window_seconds, 100);
    assert_eq!(state.window_spent, 0);
}

#[test]
fn large_direct_withdrawal_rejected() {
    let vault_id = 60;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, amount);
    configure_timelock_policy(&mut fixture, vault_id, 5_000, 100);
    let withdrawal = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        amount,
    );
    let result = send(&mut fixture.svm, &fixture.authority, withdrawal);

    assert_error_message(result, "Withdrawal amount requires a timelocked request");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.destination_token), 0);
}

#[test]
fn authorized_operator_can_create_request() {
    let vault_id = 61;
    let amount = 50_000;
    let (mut fixture, vault, _, _) = funded_fixture(vault_id, amount);
    configure_timelock_policy(&mut fixture, vault_id, 5_000, 100);
    let operator = fixture.attacker.pubkey();
    let role = role_address(vault, operator);
    let grant = grant_role_instruction(&fixture, vault_id, operator, PERMISSION_REQUEST_WITHDRAWAL);
    send(&mut fixture.svm, &fixture.authority, grant).unwrap();
    let request_address = withdrawal_address(vault, 0);
    let request = request_withdrawal_instruction(
        &fixture,
        vault_id,
        operator,
        fixture.destination_token,
        0,
        amount,
        Some(role),
    );
    send(&mut fixture.svm, &fixture.attacker, request).unwrap();

    let state = withdrawal_state(&fixture.svm, request_address);
    assert_eq!(state.proposer, operator);
    assert_eq!(state.amount, amount);
    assert_eq!(state.status, WithdrawalStatus::Pending);
    assert_eq!(vault_state(&fixture.svm, vault).next_withdrawal_id, 1);
}

#[test]
fn unauthorized_user_cannot_create_request() {
    let vault_id = 62;
    let amount = 50_000;
    let (mut fixture, vault, _, _) = funded_fixture(vault_id, amount);
    configure_timelock_policy(&mut fixture, vault_id, 5_000, 100);
    let request_address = withdrawal_address(vault, 0);
    let request = request_withdrawal_instruction(
        &fixture,
        vault_id,
        fixture.attacker.pubkey(),
        fixture.destination_token,
        0,
        amount,
        None,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, request);

    assert_error_message(result, "Caller lacks the required vault permission");
    assert!(fixture.svm.get_account(&request_address).is_none());
    assert_eq!(vault_state(&fixture.svm, vault).next_withdrawal_id, 0);
}

#[test]
fn execution_before_timelock_rejected() {
    let vault_id = 63;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, execute);

    assert_error_message(result, "Withdrawal timelock has not elapsed");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Pending
    );
}

#[test]
fn execution_after_timelock_succeeds() {
    let vault_id = 64;
    let amount = 50_000;
    let (mut fixture, _, vault_asset, vault_token, request) =
        requested_fixture(vault_id, amount, 100);
    let execute_after = withdrawal_state(&fixture.svm, request).execute_after;
    set_clock(&mut fixture.svm, execute_after);
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    send(&mut fixture.svm, &fixture.attacker, execute).unwrap();

    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount
    );
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Executed
    );
    assert_eq!(asset_state(&fixture.svm, vault_asset).window_spent, amount);
}

#[test]
fn recipient_cannot_change() {
    let vault_id = 65;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let execute_after = withdrawal_state(&fixture.svm, request).execute_after;
    set_clock(&mut fixture.svm, execute_after);
    let alternate_recipient = Pubkey::new_unique();
    set_token_account(
        &mut fixture.svm,
        alternate_recipient,
        fixture.mint,
        fixture.attacker.pubkey(),
        0,
    );
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        alternate_recipient,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, execute);

    assert_error_message(result, "Withdrawal recipient account does not match");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
    assert_eq!(token_balance(&fixture.svm, alternate_recipient), 0);
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Pending
    );
}

#[test]
fn amount_cannot_change() {
    let vault_id = 66;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let state = withdrawal_state(&fixture.svm, request);
    assert_eq!(state.amount, amount);
    set_clock(&mut fixture.svm, state.execute_after);
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    assert_eq!(
        execute.data.len(),
        8,
        "execute carries no mutable amount argument"
    );
    send(&mut fixture.svm, &fixture.attacker, execute).unwrap();

    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount
    );
}

#[test]
fn mint_cannot_change() {
    let vault_id = 67;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let execute_after = withdrawal_state(&fixture.svm, request).execute_after;
    set_clock(&mut fixture.svm, execute_after);
    let wrong_mint = Pubkey::new_unique();
    set_mint(&mut fixture.svm, wrong_mint, fixture.authority.pubkey(), 0);
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        wrong_mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, execute);

    assert!(result.is_err());
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Pending
    );
}

#[test]
fn executed_request_cannot_execute_again() {
    let vault_id = 68;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let execute_after = withdrawal_state(&fixture.svm, request).execute_after;
    set_clock(&mut fixture.svm, execute_after);
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    send(&mut fixture.svm, &fixture.attacker, execute).unwrap();
    let second = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, second);

    assert_error_message(result, "Withdrawal request is not pending");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount
    );
}

#[test]
fn cancelled_request_cannot_execute() {
    let vault_id = 69;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let state = withdrawal_state(&fixture.svm, request);
    let cancel =
        cancel_withdrawal_instruction(&fixture, vault_id, 0, fixture.authority.pubkey(), None);
    send(&mut fixture.svm, &fixture.authority, cancel).unwrap();
    set_clock(&mut fixture.svm, state.execute_after);
    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, execute);

    assert_error_message(result, "Withdrawal request is not pending");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Cancelled
    );
}

#[test]
fn guardian_can_cancel() {
    let vault_id = 70;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 100);
    let cancel =
        cancel_withdrawal_instruction(&fixture, vault_id, 0, fixture.guardian.pubkey(), None);
    send(&mut fixture.svm, &fixture.guardian, cancel).unwrap();

    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Cancelled
    );
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
}

#[test]
fn policy_change_does_not_shorten_existing_request() {
    let vault_id = 71;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 1_000);
    let immutable = withdrawal_state(&fixture.svm, request);
    let update = update_full_policy_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.mint,
        INITIAL_DEPOSITOR_BALANCE,
        INITIAL_DEPOSITOR_BALANCE,
        86_400,
        5_000,
        1,
        3_600,
        None,
    );
    send(&mut fixture.svm, &fixture.authority, update).unwrap();
    assert_eq!(
        withdrawal_state(&fixture.svm, request).execute_after,
        immutable.execute_after
    );
    set_clock(&mut fixture.svm, immutable.created_at + 1);
    let early = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, early);
    assert_error_message(result, "Withdrawal timelock has not elapsed");
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);

    set_clock(&mut fixture.svm, immutable.execute_after);
    let mature = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    send(&mut fixture.svm, &fixture.attacker, mature).unwrap();
    assert_eq!(
        token_balance(&fixture.svm, fixture.destination_token),
        amount
    );
}

#[test]
fn vault_guardian_can_pause_but_only_authority_can_unpause() {
    let vault_id = 80;
    let (mut fixture, vault, _, _) = registered_fixture(vault_id);
    let pause = pause_vault_instruction(&fixture, vault_id, fixture.guardian.pubkey());
    send(&mut fixture.svm, &fixture.guardian, pause).unwrap();
    assert!(vault_state(&fixture.svm, vault).paused);

    let guardian_unpause = unpause_vault_instruction(&fixture, vault_id, fixture.guardian.pubkey());
    let result = send(&mut fixture.svm, &fixture.guardian, guardian_unpause);
    assert_error_message(result, "Only the vault authority can unpause");
    assert!(vault_state(&fixture.svm, vault).paused);

    let authority_unpause =
        unpause_vault_instruction(&fixture, vault_id, fixture.authority.pubkey());
    send(&mut fixture.svm, &fixture.authority, authority_unpause).unwrap();
    assert!(!vault_state(&fixture.svm, vault).paused);
}

#[test]
fn random_wallet_cannot_change_local_pause_state() {
    let vault_id = 81;
    let (mut fixture, vault, _, _) = registered_fixture(vault_id);
    let pause = pause_vault_instruction(&fixture, vault_id, fixture.attacker.pubkey());
    let result = send(&mut fixture.svm, &fixture.attacker, pause);

    assert_error_message(result, "Caller cannot pause this vault");
    assert!(!vault_state(&fixture.svm, vault).paused);
}

#[test]
fn local_pause_blocks_new_outflows_but_allows_deposits() {
    let vault_id = 82;
    let (mut fixture, vault, _, vault_token) = funded_fixture(vault_id, 1_000);
    configure_timelock_policy(&mut fixture, vault_id, 100, 60);
    let pause = pause_vault_instruction(&fixture, vault_id, fixture.guardian.pubkey());
    send(&mut fixture.svm, &fixture.guardian, pause).unwrap();

    let withdraw = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        50,
    );
    let result = send(&mut fixture.svm, &fixture.authority, withdraw);
    assert_error_message(result, "Vault is paused");

    let request = request_withdrawal_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        0,
        500,
        None,
    );
    let result = send(&mut fixture.svm, &fixture.authority, request);
    assert_error_message(result, "Vault is paused");

    let deposit = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        100,
    );
    send(&mut fixture.svm, &fixture.depositor, deposit).unwrap();
    assert_eq!(token_balance(&fixture.svm, vault_token), 1_100);
    assert_eq!(vault_state(&fixture.svm, vault).next_withdrawal_id, 0);
}

#[test]
fn local_pause_blocks_execution_but_allows_cancellation() {
    let vault_id = 83;
    let amount = 50_000;
    let (mut fixture, vault, _, vault_token, request) = requested_fixture(vault_id, amount, 60);
    let request_state = withdrawal_state(&fixture.svm, request);
    let pause = pause_vault_instruction(&fixture, vault_id, fixture.guardian.pubkey());
    send(&mut fixture.svm, &fixture.guardian, pause).unwrap();
    set_clock(&mut fixture.svm, request_state.execute_after);

    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, execute);
    assert_error_message(result, "Vault is paused");

    let cancel =
        cancel_withdrawal_instruction(&fixture, vault_id, 0, fixture.guardian.pubkey(), None);
    send(&mut fixture.svm, &fixture.guardian, cancel).unwrap();
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Cancelled
    );
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
    assert!(vault_state(&fixture.svm, vault).paused);
}

#[test]
fn protocol_vault_outflow_pause_blocks_transfers_but_not_deposits() {
    let vault_id = 84;
    let (mut fixture, _, _, vault_token) = funded_fixture(vault_id, 1_000);
    let pause = set_protocol_pause_instruction(fixture.guardian.pubkey(), PAUSE_VAULT_OUTFLOW);
    send(&mut fixture.svm, &fixture.guardian, pause).unwrap();

    let withdraw = withdraw_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        100,
    );
    let result = send(&mut fixture.svm, &fixture.authority, withdraw);
    assert_error_message(result, "Protocol operation is paused");

    let request = request_withdrawal_instruction(
        &fixture,
        vault_id,
        fixture.authority.pubkey(),
        fixture.destination_token,
        0,
        500,
        None,
    );
    let result = send(&mut fixture.svm, &fixture.authority, request);
    assert_error_message(result, "Protocol operation is paused");

    let deposit = deposit_instruction(
        &fixture,
        vault_id,
        fixture.depositor.pubkey(),
        fixture.depositor_token,
        100,
    );
    send(&mut fixture.svm, &fixture.depositor, deposit).unwrap();
    assert_eq!(token_balance(&fixture.svm, vault_token), 1_100);
    assert_eq!(
        protocol_state(&fixture.svm).pause_flags,
        PAUSE_VAULT_OUTFLOW
    );
}

#[test]
fn protocol_vault_outflow_pause_blocks_execution_but_not_cancellation() {
    let vault_id = 86;
    let amount = 50_000;
    let (mut fixture, _, _, vault_token, request) = requested_fixture(vault_id, amount, 60);
    let request_state = withdrawal_state(&fixture.svm, request);
    let pause = set_protocol_pause_instruction(fixture.guardian.pubkey(), PAUSE_VAULT_OUTFLOW);
    send(&mut fixture.svm, &fixture.guardian, pause).unwrap();
    set_clock(&mut fixture.svm, request_state.execute_after);

    let execute = execute_withdrawal_instruction(
        &fixture,
        vault_id,
        0,
        fixture.attacker.pubkey(),
        fixture.mint,
        fixture.destination_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, execute);
    assert_error_message(result, "Protocol operation is paused");

    let cancel =
        cancel_withdrawal_instruction(&fixture, vault_id, 0, fixture.guardian.pubkey(), None);
    send(&mut fixture.svm, &fixture.guardian, cancel).unwrap();
    assert_eq!(
        withdrawal_state(&fixture.svm, request).status,
        WithdrawalStatus::Cancelled
    );
    assert_eq!(token_balance(&fixture.svm, vault_token), amount * 2);
}

#[test]
fn protocol_vault_config_pause_blocks_configuration_only() {
    let mut fixture = Fixture::new();
    let pause = set_protocol_pause_instruction(fixture.guardian.pubkey(), PAUSE_VAULT_CONFIG);
    send(&mut fixture.svm, &fixture.guardian, pause).unwrap();
    let blocked_create =
        create_vault_instruction(fixture.authority.pubkey(), 85, fixture.guardian.pubkey());
    let result = send(&mut fixture.svm, &fixture.authority, blocked_create);
    assert_error_message(result, "Protocol operation is paused");

    let clear = set_protocol_pause_instruction(fixture.authority.pubkey(), 0);
    send(&mut fixture.svm, &fixture.authority, clear).unwrap();
    let create =
        create_vault_instruction(fixture.authority.pubkey(), 85, fixture.guardian.pubkey());
    send(&mut fixture.svm, &fixture.authority, create).unwrap();

    let pause_again = set_protocol_pause_instruction(fixture.guardian.pubkey(), PAUSE_VAULT_CONFIG);
    send(&mut fixture.svm, &fixture.guardian, pause_again).unwrap();
    let register = register_asset_instruction(&fixture, fixture.authority.pubkey(), 85);
    let result = send(&mut fixture.svm, &fixture.authority, register);
    assert_error_message(result, "Protocol operation is paused");
}
