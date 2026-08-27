use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{program_option::COption, program_pack::Pack, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token_interface::spl_token_2022::{
        self,
        extension::{
            non_transferable::NonTransferable, permanent_delegate::PermanentDelegate,
            transfer_fee::TransferFeeConfig, transfer_hook::TransferHook,
            BaseStateWithExtensionsMut, ExtensionType, StateWithExtensions, StateWithExtensionsMut,
        },
        state::{Account as TokenAccount, AccountState, Mint},
    },
    iron_vault::{state::Escrow, ID},
    litesvm::{types::TransactionResult, LiteSVM},
    solana_account::Account as SolanaAccount,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const DECIMALS: u8 = 6;
const INITIAL_BALANCE: u64 = 1_000_000;

#[derive(Clone, Copy)]
enum MintKind {
    Vanilla,
    PermanentDelegate,
    TransferHook,
    TransferFee,
    NonTransferable,
}

struct Fixture {
    svm: LiteSVM,
    maker: Keypair,
    recipient: Pubkey,
    mint: Pubkey,
    maker_token: Pubkey,
    token_program: Pubkey,
}

impl Fixture {
    fn new(token_program: Pubkey, kind: MintKind) -> Self {
        let maker = Keypair::new();
        let recipient = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let maker_token = Pubkey::new_unique();
        let mut svm = LiteSVM::new();
        svm.add_program(ID, include_bytes!("../../../target/deploy/iron_vault.so"))
            .unwrap();
        svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap();

        let initialize = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
            ID,
            &iron_vault::instruction::InitializeProtocol {
                admin: maker.pubkey(),
                guardian: recipient,
            }
            .data(),
            iron_vault::accounts::InitializeProtocol {
                initializer: maker.pubkey(),
                protocol_config: protocol_address(),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        );
        send(&mut svm, &maker, initialize).unwrap();

        let mint_data = if token_program == spl_token_2022::ID {
            token_2022_mint_data(maker.pubkey(), kind)
        } else {
            legacy_mint_data(maker.pubkey())
        };
        set_token_program_account(&mut svm, mint, token_program, mint_data);
        set_token_program_account(
            &mut svm,
            maker_token,
            token_program,
            token_account_data(mint, maker.pubkey(), INITIAL_BALANCE),
        );

        Self {
            svm,
            maker,
            recipient,
            mint,
            maker_token,
            token_program,
        }
    }

    fn create(
        &self,
        escrow_id: u64,
        amount: u64,
    ) -> anchor_lang::solana_program::instruction::Instruction {
        let (escrow, escrow_token) = escrow_addresses(self.maker.pubkey(), escrow_id);
        let now: Clock = self.svm.get_sysvar();
        anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
            ID,
            &iron_vault::instruction::CreateEscrow {
                escrow_id,
                recipient: self.recipient,
                amount,
                expires_at: now.unix_timestamp + 600,
            }
            .data(),
            iron_vault::accounts::CreateEscrow {
                maker: self.maker.pubkey(),
                protocol_config: protocol_address(),
                mint: self.mint,
                maker_token: self.maker_token,
                escrow,
                escrow_token,
                token_program: self.token_program,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        )
    }
}

fn legacy_mint_data(authority: Pubkey) -> Vec<u8> {
    let mut data = vec![0; Mint::LEN];
    Mint::pack(base_mint(authority), &mut data).unwrap();
    data
}

fn token_2022_mint_data(authority: Pubkey, kind: MintKind) -> Vec<u8> {
    let extensions = match kind {
        MintKind::Vanilla => vec![],
        MintKind::PermanentDelegate => vec![ExtensionType::PermanentDelegate],
        MintKind::TransferHook => vec![ExtensionType::TransferHook],
        MintKind::TransferFee => vec![ExtensionType::TransferFeeConfig],
        MintKind::NonTransferable => vec![ExtensionType::NonTransferable],
    };
    if extensions.is_empty() {
        return legacy_mint_data(authority);
    }

    let length = ExtensionType::try_calculate_account_len::<Mint>(&extensions).unwrap();
    let mut data = vec![0; length];
    let mut state = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut data).unwrap();
    match kind {
        MintKind::Vanilla => unreachable!(),
        MintKind::PermanentDelegate => {
            state.init_extension::<PermanentDelegate>(true).unwrap();
        }
        MintKind::TransferHook => {
            state.init_extension::<TransferHook>(true).unwrap();
        }
        MintKind::TransferFee => {
            state.init_extension::<TransferFeeConfig>(true).unwrap();
        }
        MintKind::NonTransferable => {
            state.init_extension::<NonTransferable>(true).unwrap();
        }
    }
    state.base = base_mint(authority);
    state.pack_base();
    state.init_account_type().unwrap();
    data
}

fn base_mint(authority: Pubkey) -> Mint {
    Mint {
        mint_authority: COption::Some(authority),
        supply: INITIAL_BALANCE,
        decimals: DECIMALS,
        is_initialized: true,
        freeze_authority: COption::None,
    }
}

fn token_account_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let token = TokenAccount {
        mint,
        owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut data = vec![0; TokenAccount::LEN];
    TokenAccount::pack(token, &mut data).unwrap();
    data
}

fn set_token_program_account(svm: &mut LiteSVM, address: Pubkey, owner: Pubkey, data: Vec<u8>) {
    svm.set_account(
        address,
        SolanaAccount {
            lamports: svm.minimum_balance_for_rent_exemption(data.len()),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn protocol_address() -> Pubkey {
    Pubkey::find_program_address(&[b"protocol"], &ID).0
}

fn escrow_addresses(maker: Pubkey, escrow_id: u64) -> (Pubkey, Pubkey) {
    let escrow =
        Pubkey::find_program_address(&[b"escrow", maker.as_ref(), &escrow_id.to_le_bytes()], &ID).0;
    let escrow_token = Pubkey::find_program_address(&[b"escrow_token", escrow.as_ref()], &ID).0;
    (escrow, escrow_token)
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
    StateWithExtensions::<TokenAccount>::unpack(&account.data)
        .unwrap()
        .base
        .amount
}

fn assert_extension_rejected(kind: MintKind, escrow_id: u64) {
    let mut fixture = Fixture::new(spl_token_2022::ID, kind);
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = fixture.create(escrow_id, 100);
    let result = send(&mut fixture.svm, &fixture.maker, instruction);
    let failure = result.expect_err("extension-bearing mint unexpectedly accepted");
    assert!(
        failure
            .meta
            .logs
            .iter()
            .any(|line| line.contains("Token-2022 mint extensions are not supported")),
        "unexpected logs:\n{}",
        failure.meta.pretty_logs()
    );
    assert!(fixture.svm.get_account(&escrow).is_none());
    assert!(fixture.svm.get_account(&escrow_token).is_none());
    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_BALANCE
    );
}

fn assert_supported(token_program: Pubkey, escrow_id: u64) {
    let mut fixture = Fixture::new(token_program, MintKind::Vanilla);
    let amount = 250_000;
    let (escrow, escrow_token) = escrow_addresses(fixture.maker.pubkey(), escrow_id);
    let instruction = fixture.create(escrow_id, amount);
    send(&mut fixture.svm, &fixture.maker, instruction).unwrap();

    assert_eq!(
        token_balance(&fixture.svm, fixture.maker_token),
        INITIAL_BALANCE - amount
    );
    assert_eq!(token_balance(&fixture.svm, escrow_token), amount);
    let account = fixture.svm.get_account(&escrow).unwrap();
    let state = Escrow::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(state.token_program, token_program);
}

#[test]
fn token2022_vanilla_supported() {
    assert_supported(spl_token_2022::ID, 10_001);
}

#[test]
fn legacy_spl_still_supported() {
    assert_supported(anchor_spl::token::spl_token::ID, 10_002);
}

#[test]
fn permanent_delegate_mint_rejected() {
    assert_extension_rejected(MintKind::PermanentDelegate, 10_003);
}

#[test]
fn transfer_hook_mint_rejected() {
    assert_extension_rejected(MintKind::TransferHook, 10_004);
}

#[test]
fn transfer_fee_mint_rejected() {
    assert_extension_rejected(MintKind::TransferFee, 10_005);
}

#[test]
fn non_transferable_mint_rejected() {
    assert_extension_rejected(MintKind::NonTransferable, 10_006);
}
