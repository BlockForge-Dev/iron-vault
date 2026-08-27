use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{
            program_option::COption, program_pack::Pack, system_program, sysvar::SysvarId,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token::spl_token::{
        self,
        state::{Account as SplTokenAccount, AccountState, Mint as SplMint},
    },
    iron_vault::{
        constants::{PAUSE_ESCROW_CREATE, PAUSE_ESCROW_RELEASE},
        state::{Escrow, EscrowStatus, ProtocolConfig},
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
const INITIAL_MAKER_BALANCE: u64 = 1_000_000;

struct Fixture {
    svm: LiteSVM,
    maker: Keypair,
    recipient_signer: Keypair,
    recipient: Pubkey,
    attacker: Keypair,
    mint: Pubkey,
    maker_token: Pubkey,
    recipient_token: Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let maker = Keypair::new();
        let recipient_signer = Keypair::new();
        let recipient = recipient_signer.pubkey();
        let attacker = Keypair::new();
        let mint = Pubkey::new_unique();
        let maker_token = Pubkey::new_unique();
        let recipient_token = Pubkey::new_unique();
        let mut svm = LiteSVM::new();

        svm.add_program(ID, include_bytes!("../../../target/deploy/iron_vault.so"))
            .unwrap();
        svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&recipient, 10_000_000_000).unwrap();
        svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
        let initialize = initialize_protocol_instruction(
            maker.pubkey(),
            maker.pubkey(),
            recipient_signer.pubkey(),
        );
        send(&mut svm, &maker, initialize).unwrap();
        set_mint(&mut svm, mint, maker.pubkey(), INITIAL_MAKER_BALANCE);
        set_token_account(
            &mut svm,
            maker_token,
            mint,
            maker.pubkey(),
            INITIAL_MAKER_BALANCE,
        );
        set_token_account(&mut svm, recipient_token, mint, recipient, 0);

        Self {
            svm,
            maker,
            recipient_signer,
            recipient,
            attacker,
            mint,
            maker_token,
            recipient_token,
        }
    }

    fn now(&self) -> i64 {
        let clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp
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

fn escrow_addresses(maker: Pubkey, escrow_id: u64) -> (Pubkey, Pubkey) {
    let (escrow, _) =
        Pubkey::find_program_address(&[b"escrow", maker.as_ref(), &escrow_id.to_le_bytes()], &ID);
    let (escrow_token, _) = Pubkey::find_program_address(&[b"escrow_token", escrow.as_ref()], &ID);
    (escrow, escrow_token)
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

fn create_instruction(
    fixture: &Fixture,
    escrow_id: u64,
    recipient: Pubkey,
    amount: u64,
    expires_at: i64,
) -> anchor_lang::solana_program::instruction::Instruction {
    create_instruction_with_token_program(
        fixture,
        escrow_id,
        recipient,
        amount,
        expires_at,
        spl_token::ID,
    )
}

fn create_instruction_with_token_program(
    fixture: &Fixture,
    escrow_id: u64,
    recipient: Pubkey,
    amount: u64,
    expires_at: i64,
    token_program: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::CreateEscrow {
            escrow_id,
            recipient,
            amount,
            expires_at,
        }
        .data(),
        iron_vault::accounts::CreateEscrow {
            maker: fixture.maker.pubkey(),
            protocol_config: protocol_address(),
            mint: fixture.mint,
            maker_token: fixture.maker_token,
            escrow,
            escrow_token,
            token_program,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn funded_fixture(escrow_id: u64, amount: u64) -> (Fixture, Pubkey, Pubkey) {
    let mut fixture = Fixture::new();
    let expires_at = fixture.now() + 600;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction =
        create_instruction(&fixture, escrow_id, fixture.recipient, amount, expires_at);
    send(&mut fixture.svm, &fixture.maker, instruction).unwrap();
    (fixture, escrow, escrow_token)
}

fn release_instruction(
    fixture: &Fixture,
    escrow_id: u64,
    signer: Pubkey,
    recipient_token: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::ReleaseEscrow {}.data(),
        iron_vault::accounts::ReleaseEscrow {
            maker: signer,
            protocol_config: protocol_address(),
            escrow,
            mint: fixture.mint,
            escrow_token,
            recipient_token,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    )
}

fn refund_instruction(
    fixture: &Fixture,
    escrow_id: u64,
    caller: Pubkey,
    maker_destination: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        ID,
        &iron_vault::instruction::RefundEscrow {}.data(),
        iron_vault::accounts::RefundEscrow {
            caller,
            escrow,
            mint: fixture.mint,
            escrow_token,
            maker_destination,
            token_program: spl_token::ID,
            clock: Clock::id(),
        }
        .to_account_metas(None),
    )
}

fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock: Clock = svm.get_sysvar();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
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

fn escrow_state(svm: &LiteSVM, address: Pubkey) -> Escrow {
    let account = svm.get_account(&address).unwrap();
    Escrow::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn protocol_state(svm: &LiteSVM) -> ProtocolConfig {
    let account = svm.get_account(&protocol_address()).unwrap();
    ProtocolConfig::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn assert_error_message(result: TransactionResult, expected: &str) {
    let failure = result.expect_err("transaction unexpectedly succeeded");
    assert!(
        failure.meta.logs.iter().any(|line| line.contains(expected)),
        "expected error containing {expected:?}, got logs:\n{}",
        failure.meta.pretty_logs()
    );
}

#[test]
fn initialize_still_dispatches() {
    let program_id = iron_vault::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    svm.add_program(
        program_id,
        include_bytes!("../../../target/deploy/iron_vault.so"),
    )
    .unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        program_id,
        &iron_vault::instruction::Initialize {}.data(),
        iron_vault::accounts::Initialize {}.to_account_metas(None),
    );
    send(&mut svm, &payer, instruction).unwrap();
}

#[test]
fn protocol_initialization_stores_distinct_authorities() {
    let fixture = Fixture::new();
    let config = protocol_state(&fixture.svm);

    assert_eq!(config.version, 1);
    assert_eq!(config.admin, fixture.maker.pubkey());
    assert_eq!(config.guardian, fixture.recipient_signer.pubkey());
    assert_eq!(config.pause_flags, 0);
}

#[test]
fn protocol_initialization_rejects_invalid_authorities() {
    for case in 0..3 {
        let initializer = Keypair::new();
        let candidate_guardian = Pubkey::new_unique();
        let (admin, guardian) = match case {
            0 => (Pubkey::default(), candidate_guardian),
            1 => (initializer.pubkey(), Pubkey::default()),
            _ => (initializer.pubkey(), initializer.pubkey()),
        };
        let mut svm = LiteSVM::new();
        svm.add_program(ID, include_bytes!("../../../target/deploy/iron_vault.so"))
            .unwrap();
        svm.airdrop(&initializer.pubkey(), 1_000_000_000).unwrap();
        let instruction = initialize_protocol_instruction(initializer.pubkey(), admin, guardian);
        let result = send(&mut svm, &initializer, instruction);

        assert_error_message(result, "Protocol authority configuration is invalid");
        assert!(svm.get_account(&protocol_address()).is_none());
    }
}

#[test]
fn guardian_can_add_pause_flags_but_cannot_clear_them() {
    let mut fixture = Fixture::new();
    let guardian = fixture.recipient_signer.pubkey();
    let add_create = set_protocol_pause_instruction(guardian, PAUSE_ESCROW_CREATE);
    send(&mut fixture.svm, &fixture.recipient_signer, add_create).unwrap();
    let add_release =
        set_protocol_pause_instruction(guardian, PAUSE_ESCROW_CREATE | PAUSE_ESCROW_RELEASE);
    send(&mut fixture.svm, &fixture.recipient_signer, add_release).unwrap();

    let clear_create = set_protocol_pause_instruction(guardian, PAUSE_ESCROW_RELEASE);
    let result = send(&mut fixture.svm, &fixture.recipient_signer, clear_create);
    assert_error_message(result, "Protocol guardian cannot clear pause flags");
    assert_eq!(
        protocol_state(&fixture.svm).pause_flags,
        PAUSE_ESCROW_CREATE | PAUSE_ESCROW_RELEASE
    );
}

#[test]
fn admin_can_clear_pause_flags_but_random_wallet_cannot_manage_them() {
    let mut fixture = Fixture::new();
    let guardian = fixture.recipient_signer.pubkey();
    let pause = set_protocol_pause_instruction(guardian, PAUSE_ESCROW_CREATE);
    send(&mut fixture.svm, &fixture.recipient_signer, pause).unwrap();

    let unauthorized =
        set_protocol_pause_instruction(fixture.attacker.pubkey(), PAUSE_ESCROW_RELEASE);
    let result = send(&mut fixture.svm, &fixture.attacker, unauthorized);
    assert_error_message(result, "Caller cannot manage protocol pause flags");

    let clear = set_protocol_pause_instruction(fixture.maker.pubkey(), 0);
    send(&mut fixture.svm, &fixture.maker, clear).unwrap();
    assert_eq!(protocol_state(&fixture.svm).pause_flags, 0);
}

#[test]
fn unknown_pause_flags_are_rejected() {
    let mut fixture = Fixture::new();
    let instruction = set_protocol_pause_instruction(fixture.maker.pubkey(), 1 << 31);
    let result = send(&mut fixture.svm, &fixture.maker, instruction);

    assert_error_message(result, "Protocol pause mask contains unknown flags");
    assert_eq!(protocol_state(&fixture.svm).pause_flags, 0);
}

#[test]
fn escrow_create_pause_is_directional_and_admin_can_resume() {
    let mut fixture = Fixture::new();
    let pause =
        set_protocol_pause_instruction(fixture.recipient_signer.pubkey(), PAUSE_ESCROW_CREATE);
    send(&mut fixture.svm, &fixture.recipient_signer, pause).unwrap();
    let expires_at = fixture.now() + 600;
    let blocked = create_instruction(&fixture, 500, fixture.recipient, 100, expires_at);
    let result = send(&mut fixture.svm, &fixture.maker, blocked);
    assert_error_message(result, "Protocol operation is paused");
    assert!(fixture
        .svm
        .get_account(&escrow_addresses(fixture.maker.pubkey(), 500).0)
        .is_none());

    let clear = set_protocol_pause_instruction(fixture.maker.pubkey(), 0);
    send(&mut fixture.svm, &fixture.maker, clear).unwrap();
    let resumed = create_instruction(&fixture, 500, fixture.recipient, 100, expires_at);
    send(&mut fixture.svm, &fixture.maker, resumed).unwrap();
}

#[test]
fn release_pause_never_blocks_expired_refund() {
    let amount = 100;
    let (mut fixture, escrow, escrow_token) = funded_fixture(501, amount);
    let pause =
        set_protocol_pause_instruction(fixture.recipient_signer.pubkey(), PAUSE_ESCROW_RELEASE);
    send(&mut fixture.svm, &fixture.recipient_signer, pause).unwrap();
    let release = release_instruction(
        &fixture,
        501,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.maker, release);
    assert_error_message(result, "Protocol operation is paused");
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);

    let expiry = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expiry);
    let refund = refund_instruction(
        &fixture,
        501,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    send(&mut fixture.svm, &fixture.attacker, refund).unwrap();
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Refunded
    );
}

#[test]
fn create_valid_escrow_succeeds() {
    let mut fixture = Fixture::new();
    let escrow_id = 7;
    let amount = 400_000;
    let expires_at = fixture.now() + 600;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);

    let create = create_instruction(&fixture, escrow_id, fixture.recipient, amount, expires_at);
    let metadata = send(&mut fixture.svm, &fixture.maker, create).unwrap();
    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE - amount
    );
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);

    let state = escrow_state(&fixture.svm, escrow);
    assert_eq!(state.maker, fixture.maker.pubkey());
    assert_eq!(state.recipient, fixture.recipient);
    assert_eq!(state.mint, fixture.mint);
    assert_eq!(state.token_program, spl_token::ID);
    assert_eq!(state.escrow_id, escrow_id);
    assert_eq!(state.amount, amount);
    assert_eq!(state.expires_at, expires_at);
    assert_eq!(state.status, EscrowStatus::Funded);
    let custody = fixture.svm.get_account(&escrow_token).unwrap();
    assert_eq!(
        SplTokenAccount::unpack(&custody.data).unwrap().owner,
        escrow
    );
}

#[test]
fn zero_amount_rejected() {
    let mut fixture = Fixture::new();
    let escrow_id = 8;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = create_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        0,
        fixture.now() + 100,
    );
    let result = send(&mut fixture.svm, &fixture.maker, instruction);

    assert_error_message(result, "Escrow amount must be greater than zero");
    assert!(fixture.svm.get_account(&escrow).is_none());
    assert!(fixture.svm.get_account(&escrow_token).is_none());
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
}

#[test]
fn maker_must_sign() {
    let fixture = Fixture::new();
    let instruction = create_instruction(&fixture, 9, fixture.recipient, 1, fixture.now() + 100);
    assert!(instruction.accounts[0].is_signer);

    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&fixture.attacker.pubkey()),
        &fixture.svm.latest_blockhash(),
    );
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&fixture.attacker]);
    assert!(
        transaction.is_err(),
        "transaction accepted without maker signature"
    );
}

#[test]
fn wrong_source_owner_rejected() {
    let mut fixture = Fixture::new();
    set_token_account(
        &mut fixture.svm,
        fixture.maker_token,
        fixture.mint,
        fixture.attacker.pubkey(),
        INITIAL_MAKER_BALANCE,
    );
    let instruction = create_instruction(&fixture, 10, fixture.recipient, 1, fixture.now() + 100);
    let result = send(&mut fixture.svm, &fixture.maker, instruction);

    assert_error_message(result, "Maker does not own the source token account");
}

#[test]
fn wrong_source_mint_rejected() {
    let mut fixture = Fixture::new();
    let wrong_mint = Pubkey::new_unique();
    set_mint(
        &mut fixture.svm,
        wrong_mint,
        fixture.maker.pubkey(),
        INITIAL_MAKER_BALANCE,
    );
    set_token_account(
        &mut fixture.svm,
        fixture.maker_token,
        wrong_mint,
        fixture.maker.pubkey(),
        INITIAL_MAKER_BALANCE,
    );
    let instruction = create_instruction(&fixture, 11, fixture.recipient, 1, fixture.now() + 100);
    let result = send(&mut fixture.svm, &fixture.maker, instruction);

    assert_error_message(result, "Source token account mint does not match");
}

#[test]
fn fake_token_program_rejected() {
    let mut fixture = Fixture::new();
    let escrow_id = 12;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = create_instruction_with_token_program(
        &fixture,
        escrow_id,
        fixture.recipient,
        1,
        fixture.now() + 100,
        system_program::ID,
    );
    let result = send(&mut fixture.svm, &fixture.maker, instruction);

    assert!(result.is_err());
    assert!(fixture.svm.get_account(&escrow).is_none());
    assert!(fixture.svm.get_account(&escrow_token).is_none());
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
}

#[test]
fn duplicate_escrow_id_rejected() {
    let mut fixture = Fixture::new();
    let escrow_id = 13;
    let amount = 25;
    let instruction = create_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        amount,
        fixture.now() + 100,
    );
    send(&mut fixture.svm, &fixture.maker, instruction).unwrap();

    let duplicate = create_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        amount,
        fixture.now() + 100,
    );
    let result = send(&mut fixture.svm, &fixture.maker, duplicate);
    let (_, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);

    assert!(result.is_err());
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE - amount
    );
}

#[test]
fn invalid_expiry_rejected() {
    let mut fixture = Fixture::new();
    let escrow_id = 14;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = create_instruction(&fixture, escrow_id, fixture.recipient, 1, fixture.now());
    let result = send(&mut fixture.svm, &fixture.maker, instruction);

    assert_error_message(result, "Escrow expiry must be in the future");
    assert!(fixture.svm.get_account(&escrow).is_none());
    assert!(fixture.svm.get_account(&escrow_token).is_none());
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
}

#[test]
fn escrow_vault_receives_exact_amount() {
    let mut fixture = Fixture::new();
    let escrow_id = 15;
    let amount = 123_456;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = create_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        amount,
        fixture.now() + 100,
    );
    send(&mut fixture.svm, &fixture.maker, instruction).unwrap();

    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE - amount
    );
    let custody = fixture.svm.get_account(&escrow_token).unwrap();
    assert_eq!(
        SplTokenAccount::unpack(&custody.data).unwrap().owner,
        escrow
    );
}

#[test]
fn maker_can_release() {
    let escrow_id = 20;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);

    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    let metadata = send(&mut fixture.svm, &fixture.maker, release).unwrap();
    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), amount);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Released
    );
}

#[test]
fn recipient_cannot_release() {
    let escrow_id = 21;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.recipient_signer, release);

    assert!(result.is_err());
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn random_wallet_cannot_release() {
    let escrow_id = 22;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, release);

    assert!(result.is_err());
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn wrong_recipient_token_account_rejected() {
    let escrow_id = 23;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let attacker_token = Pubkey::new_unique();
    set_token_account(
        &mut fixture.svm,
        attacker_token,
        fixture.mint,
        fixture.attacker.pubkey(),
        0,
    );
    let release = release_instruction(&fixture, escrow_id, fixture.maker.pubkey(), attacker_token);
    let result = send(&mut fixture.svm, &fixture.maker, release);

    assert_error_message(
        result,
        "Release destination is not owned by the escrow recipient",
    );
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn wrong_mint_rejected() {
    let escrow_id = 24;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let wrong_mint = Pubkey::new_unique();
    let wrong_token = Pubkey::new_unique();
    set_mint(&mut fixture.svm, wrong_mint, fixture.maker.pubkey(), 0);
    set_token_account(
        &mut fixture.svm,
        wrong_token,
        wrong_mint,
        fixture.recipient,
        0,
    );
    let release = release_instruction(&fixture, escrow_id, fixture.maker.pubkey(), wrong_token);
    let result = send(&mut fixture.svm, &fixture.maker, release);

    assert_error_message(result, "Release destination mint does not match");
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn release_transfers_exact_amount() {
    let mut fixture = Fixture::new();
    let escrow_id = 25;
    let amount = 333_333;
    let recipient_before = 17;
    set_token_account(
        &mut fixture.svm,
        fixture.recipient_token,
        fixture.mint,
        fixture.recipient,
        recipient_before,
    );
    let (_, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let create = create_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        amount,
        fixture.now() + 600,
    );
    send(&mut fixture.svm, &fixture.maker, create).unwrap();
    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    send(&mut fixture.svm, &fixture.maker, release).unwrap();

    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.recipient_token),
        recipient_before + amount
    );
}

#[test]
fn released_escrow_cannot_release_again() {
    let escrow_id = 26;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    send(&mut fixture.svm, &fixture.maker, release).unwrap();

    let second_release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.maker, second_release);
    assert_error_message(result, "Escrow is not funded");
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), amount);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Released
    );
}

#[test]
fn self_recipient_rejected() {
    let mut fixture = Fixture::new();
    let instruction =
        create_instruction(&fixture, 30, fixture.maker.pubkey(), 1, fixture.now() + 100);
    let result = send(&mut fixture.svm, &fixture.maker, instruction);
    assert_error_message(result, "Escrow recipient is invalid");
}

#[test]
fn create_rejects_an_uninitialized_mint() {
    let mut fixture = Fixture::new();
    set_spl_account(&mut fixture.svm, fixture.mint, vec![0; SplMint::LEN]);
    let escrow_id = 31;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = create_instruction(
        &fixture,
        escrow_id,
        fixture.recipient,
        1,
        fixture.now() + 100,
    );
    let result = send(&mut fixture.svm, &fixture.maker, instruction);
    assert!(result.is_err());
    assert!(fixture.svm.get_account(&escrow).is_none());
    assert!(fixture.svm.get_account(&escrow_token).is_none());
}

#[test]
fn release_at_expiry_rejected() {
    let escrow_id = 32;
    let amount = 250_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;

    let mut clock: Clock = fixture.svm.get_sysvar();
    clock.unix_timestamp = expires_at;
    fixture.svm.set_sysvar(&clock);
    let expired_release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.maker, expired_release);
    assert_error_message(result, "Escrow has expired");

    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn refund_before_expiry_fails() {
    let escrow_id = 40;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let maker_balance = token_balance(&fixture.svm, fixture.maker_token);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, refund);

    assert_error_message(result, "Escrow has not expired");
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        maker_balance
    );
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn refund_at_expiry_succeeds() {
    let escrow_id = 41;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.maker_token,
    );
    let metadata = send(&mut fixture.svm, &fixture.maker, refund).unwrap();

    assert!(metadata
        .logs
        .iter()
        .any(|line| line.contains("Program data:")));
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Refunded
    );
}

#[test]
fn refund_after_expiry_succeeds() {
    let escrow_id = 42;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at + 1);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.maker_token,
    );
    send(&mut fixture.svm, &fixture.maker, refund).unwrap();

    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Refunded
    );
}

#[test]
fn random_caller_can_trigger_refund() {
    let escrow_id = 43;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    send(&mut fixture.svm, &fixture.attacker, refund).unwrap();

    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Refunded
    );
}

#[test]
fn random_caller_cannot_redirect_refund() {
    let escrow_id = 44;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at);
    let attacker_token = Pubkey::new_unique();
    set_token_account(
        &mut fixture.svm,
        attacker_token,
        fixture.mint,
        fixture.attacker.pubkey(),
        0,
    );
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        attacker_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, refund);

    assert_error_message(
        result,
        "Refund destination is not owned by the escrow maker",
    );
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(token_balance(&fixture.svm, attacker_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn refund_wrong_destination_mint_rejected() {
    let escrow_id = 48;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at);
    let wrong_mint = Pubkey::new_unique();
    let wrong_token = Pubkey::new_unique();
    set_mint(&mut fixture.svm, wrong_mint, fixture.maker.pubkey(), 0);
    set_token_account(
        &mut fixture.svm,
        wrong_token,
        wrong_mint,
        fixture.maker.pubkey(),
        0,
    );
    let refund = refund_instruction(&fixture, escrow_id, fixture.attacker.pubkey(), wrong_token);
    let result = send(&mut fixture.svm, &fixture.attacker, refund);

    assert_error_message(result, "Refund destination mint does not match");
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    assert_eq!(token_balance(&fixture.svm, wrong_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Funded
    );
}

#[test]
fn released_escrow_cannot_refund() {
    let escrow_id = 45;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    send(&mut fixture.svm, &fixture.maker, release).unwrap();
    set_clock(&mut fixture.svm, expires_at);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, refund);

    assert_error_message(result, "Escrow is not funded");
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), amount);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Released
    );
}

#[test]
fn refunded_escrow_cannot_release() {
    let escrow_id = 46;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    send(&mut fixture.svm, &fixture.attacker, refund).unwrap();
    let release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.maker, release);

    assert_error_message(result, "Escrow is not funded");
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(token_balance(&fixture.svm, fixture.recipient_token), 0);
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Refunded
    );
}

#[test]
fn refund_cannot_execute_twice() {
    let escrow_id = 47;
    let amount = 200_000;
    let (mut fixture, escrow, escrow_token) = funded_fixture(escrow_id, amount);
    let expires_at = escrow_state(&fixture.svm, escrow).expires_at;
    set_clock(&mut fixture.svm, expires_at);
    let refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    send(&mut fixture.svm, &fixture.attacker, refund).unwrap();
    let second_refund = refund_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.maker_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, second_refund);

    assert_error_message(result, "Escrow is not funded");
    assert_eq!(token_balance(&fixture.svm, escrow_token), 0);
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_MAKER_BALANCE
    );
    assert_eq!(
        escrow_state(&fixture.svm, escrow).status,
        EscrowStatus::Refunded
    );
}
