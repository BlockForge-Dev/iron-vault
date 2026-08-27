use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{program_option::COption, program_pack::Pack, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token::spl_token::{
        self,
        state::{Account as SplTokenAccount, AccountState, Mint as SplMint},
    },
    iron_vault::{
        state::{Escrow, EscrowStatus},
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
    recipient: Pubkey,
    attacker: Keypair,
    mint: Pubkey,
    maker_token: Pubkey,
    recipient_token: Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let maker = Keypair::new();
        let recipient = Pubkey::new_unique();
        let attacker = Keypair::new();
        let mint = Pubkey::new_unique();
        let maker_token = Pubkey::new_unique();
        let recipient_token = Pubkey::new_unique();
        let mut svm = LiteSVM::new();

        svm.add_program(ID, include_bytes!("../../../target/deploy/iron_vault.so"))
            .unwrap();
        svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
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

fn create_instruction(
    fixture: &Fixture,
    escrow_id: u64,
    recipient: Pubkey,
    amount: u64,
    expires_at: i64,
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
            mint: fixture.mint,
            maker_token: fixture.maker_token,
            escrow,
            escrow_token,
            token_program: spl_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
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
            escrow,
            mint: fixture.mint,
            escrow_token,
            recipient_token,
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

fn escrow_state(svm: &LiteSVM, address: Pubkey) -> Escrow {
    let account = svm.get_account(&address).unwrap();
    Escrow::try_deserialize(&mut account.data.as_slice()).unwrap()
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
fn create_and_release_moves_the_exact_amount() {
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

    let second_release = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.maker, second_release);
    assert_error_message(result, "Escrow is not funded");
}

#[test]
fn create_rejects_invalid_terms_without_side_effects() {
    let cases = [
        (1, Pubkey::new_unique(), 0, 100_i64, "greater than zero"),
        (2, Pubkey::default(), 1, 100_i64, "recipient is invalid"),
    ];

    for (escrow_id, recipient, amount, expiry_delta, error) in cases {
        let mut fixture = Fixture::new();
        let expires_at = fixture.now() + expiry_delta;
        let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
        let instruction = create_instruction(&fixture, escrow_id, recipient, amount, expires_at);
        let result = send(&mut fixture.svm, &fixture.maker, instruction);
        assert_error_message(result, error);
        assert!(fixture.svm.get_account(&escrow).is_none());
        assert!(fixture.svm.get_account(&escrow_token).is_none());
        assert_eq!(
            token_balance(&fixture.svm, fixture.maker_token),
            INITIAL_MAKER_BALANCE
        );
    }

    let mut fixture = Fixture::new();
    let instruction = create_instruction(&fixture, 3, fixture.recipient, 1, fixture.now());
    let result = send(&mut fixture.svm, &fixture.maker, instruction);
    assert_error_message(result, "expiry must be in the future");

    let mut fixture = Fixture::new();
    let instruction =
        create_instruction(&fixture, 4, fixture.maker.pubkey(), 1, fixture.now() + 100);
    let result = send(&mut fixture.svm, &fixture.maker, instruction);
    assert_error_message(result, "recipient is invalid");
}

#[test]
fn create_rejects_source_owner_and_mint_mismatches() {
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
    assert_error_message(result, "does not own the source token account");

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

    let mut fixture = Fixture::new();
    let instruction = create_instruction(
        &fixture,
        12,
        fixture.recipient,
        INITIAL_MAKER_BALANCE + 1,
        fixture.now() + 100,
    );
    let result = send(&mut fixture.svm, &fixture.maker, instruction);
    assert_error_message(result, "insufficient funds");
}

#[test]
fn create_rejects_an_uninitialized_mint() {
    let mut fixture = Fixture::new();
    set_spl_account(&mut fixture.svm, fixture.mint, vec![0; SplMint::LEN]);
    let escrow_id = 13;
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
fn release_enforces_maker_destination_and_expiry() {
    let mut fixture = Fixture::new();
    let escrow_id = 21;
    let amount = 250_000;
    let expires_at = fixture.now() + 100;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let create = create_instruction(&fixture, escrow_id, fixture.recipient, amount, expires_at);
    send(&mut fixture.svm, &fixture.maker, create).unwrap();

    let wrong_signer_release = release_instruction(
        &fixture,
        escrow_id,
        fixture.attacker.pubkey(),
        fixture.recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.attacker, wrong_signer_release);
    assert!(result.is_err());

    let attacker_token = Pubkey::new_unique();
    set_token_account(
        &mut fixture.svm,
        attacker_token,
        fixture.mint,
        fixture.attacker.pubkey(),
        0,
    );
    let wrong_destination =
        release_instruction(&fixture, escrow_id, fixture.maker.pubkey(), attacker_token);
    let result = send(&mut fixture.svm, &fixture.maker, wrong_destination);
    assert_error_message(result, "not owned by the escrow recipient");

    let wrong_mint = Pubkey::new_unique();
    let wrong_mint_recipient_token = Pubkey::new_unique();
    set_mint(&mut fixture.svm, wrong_mint, fixture.maker.pubkey(), 0);
    set_token_account(
        &mut fixture.svm,
        wrong_mint_recipient_token,
        wrong_mint,
        fixture.recipient,
        0,
    );
    let wrong_destination_mint = release_instruction(
        &fixture,
        escrow_id,
        fixture.maker.pubkey(),
        wrong_mint_recipient_token,
    );
    let result = send(&mut fixture.svm, &fixture.maker, wrong_destination_mint);
    assert_error_message(result, "Release destination mint does not match");

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
