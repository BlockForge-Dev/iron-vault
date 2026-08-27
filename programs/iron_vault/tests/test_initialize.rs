use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

#[test]
fn initialize_dispatches_in_litesvm() {
    let program_id = iron_vault::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let program = include_bytes!("../../../target/deploy/iron_vault.so");

    svm.add_program(program_id, program).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &iron_vault::instruction::Initialize {}.data(),
        iron_vault::accounts::Initialize {}.to_account_metas(None),
    );
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&payer]).unwrap();

    let result = svm.send_transaction(transaction);
    assert!(result.is_ok(), "initialize failed: {result:?}");
}
