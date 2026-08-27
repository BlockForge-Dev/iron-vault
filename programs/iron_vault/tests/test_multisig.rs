use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    iron_vault::{state::Vault, ID as IRON_VAULT_ID},
    litesvm::{types::TransactionResult, LiteSVM},
    mock_multisig::ID as MOCK_MULTISIG_ID,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

fn protocol_address() -> Pubkey {
    Pubkey::find_program_address(&[b"protocol"], &IRON_VAULT_ID).0
}

fn vault_address(namespace_authority: Pubkey, vault_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"vault",
            namespace_authority.as_ref(),
            &vault_id.to_le_bytes(),
        ],
        &IRON_VAULT_ID,
    )
    .0
}

fn multisig_address(creator: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &MOCK_MULTISIG_ID).0
}

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    additional_signers: &[&Keypair],
    instruction: Instruction,
) -> TransactionResult {
    svm.expire_blockhash();
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );
    let mut signers = vec![payer];
    signers.extend_from_slice(additional_signers);
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &signers).unwrap();
    svm.send_transaction(transaction)
}

fn assert_error_message(result: TransactionResult, expected: &str) {
    let failure = result.expect_err("transaction unexpectedly succeeded");
    assert!(
        failure.meta.logs.iter().any(|line| line.contains(expected)),
        "expected error containing {expected:?}, got logs:\n{}",
        failure.meta.pretty_logs()
    );
}

fn vault_state(svm: &LiteSVM, address: Pubkey) -> Vault {
    let account = svm.get_account(&address).unwrap();
    Vault::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn mock_execute_instruction(
    multisig: Pubkey,
    signer_a: Pubkey,
    signer_b: Pubkey,
    inner: Instruction,
) -> Instruction {
    let mut instruction = Instruction::new_with_bytes(
        MOCK_MULTISIG_ID,
        &mock_multisig::instruction::Execute {
            instruction_data: inner.data,
        }
        .data(),
        mock_multisig::accounts::Execute {
            multisig,
            signer_a,
            signer_b,
            target_program: inner.program_id,
        }
        .to_account_metas(None),
    );
    instruction
        .accounts
        .extend(inner.accounts.into_iter().map(|mut meta| {
            // The multisig PDA becomes a signer only inside the approved CPI.
            meta.is_signer = false;
            meta
        }));
    instruction
}

#[test]
fn external_two_of_three_multisig_controls_rotated_vault_authority() {
    let developer = Keypair::new();
    let guardian = Keypair::new();
    let owner_a = Keypair::new();
    let owner_b = Keypair::new();
    let owner_c = Keypair::new();
    let outsider = Keypair::new();
    let owners = [owner_a.pubkey(), owner_b.pubkey(), owner_c.pubkey()];
    let vault_id = 900;
    let vault = vault_address(developer.pubkey(), vault_id);
    let multisig = multisig_address(developer.pubkey());
    let mut svm = LiteSVM::new();
    svm.add_program(
        IRON_VAULT_ID,
        include_bytes!("../../../target/deploy/iron_vault.so"),
    )
    .unwrap();
    svm.add_program(
        MOCK_MULTISIG_ID,
        include_bytes!("../../../target/deploy/mock_multisig.so"),
    )
    .unwrap();
    for signer in [
        &developer, &guardian, &owner_a, &owner_b, &owner_c, &outsider,
    ] {
        svm.airdrop(&signer.pubkey(), 10_000_000_000).unwrap();
    }

    let initialize_multisig = Instruction::new_with_bytes(
        MOCK_MULTISIG_ID,
        &mock_multisig::instruction::Initialize { owners }.data(),
        mock_multisig::accounts::Initialize {
            creator: developer.pubkey(),
            multisig,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &developer, &[], initialize_multisig).unwrap();

    let initialize_protocol = Instruction::new_with_bytes(
        IRON_VAULT_ID,
        &iron_vault::instruction::InitializeProtocol {
            admin: developer.pubkey(),
            guardian: guardian.pubkey(),
        }
        .data(),
        iron_vault::accounts::InitializeProtocol {
            initializer: developer.pubkey(),
            protocol_config: protocol_address(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &developer, &[], initialize_protocol).unwrap();

    let create_vault = Instruction::new_with_bytes(
        IRON_VAULT_ID,
        &iron_vault::instruction::CreateVault {
            vault_id,
            guardian: guardian.pubkey(),
        }
        .data(),
        iron_vault::accounts::CreateVault {
            authority: developer.pubkey(),
            protocol_config: protocol_address(),
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &developer, &[], create_vault).unwrap();

    let rotate = Instruction::new_with_bytes(
        IRON_VAULT_ID,
        &iron_vault::instruction::SetVaultAuthority {
            new_authority: multisig,
        }
        .data(),
        iron_vault::accounts::SetVaultAuthority {
            current_authority: developer.pubkey(),
            protocol_config: protocol_address(),
            vault,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &developer, &[], rotate).unwrap();
    assert_eq!(vault_state(&svm, vault).authority, multisig);

    let pause = Instruction::new_with_bytes(
        IRON_VAULT_ID,
        &iron_vault::instruction::PauseVault {}.data(),
        iron_vault::accounts::PauseVault {
            caller: guardian.pubkey(),
            vault,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &guardian, &[], pause).unwrap();

    let old_authority_unpause = Instruction::new_with_bytes(
        IRON_VAULT_ID,
        &iron_vault::instruction::UnpauseVault {}.data(),
        iron_vault::accounts::UnpauseVault {
            authority: developer.pubkey(),
            vault,
        }
        .to_account_metas(None),
    );
    let result = send(&mut svm, &developer, &[], old_authority_unpause);
    assert_error_message(result, "Only the vault authority can unpause");

    let unpause = || {
        Instruction::new_with_bytes(
            IRON_VAULT_ID,
            &iron_vault::instruction::UnpauseVault {}.data(),
            iron_vault::accounts::UnpauseVault {
                authority: multisig,
                vault,
            }
            .to_account_metas(None),
        )
    };
    let duplicate_approval =
        mock_execute_instruction(multisig, owner_a.pubkey(), owner_a.pubkey(), unpause());
    let result = send(&mut svm, &owner_a, &[], duplicate_approval);
    assert_error_message(result, "Mock multisig approvals must be distinct");

    let nonmember =
        mock_execute_instruction(multisig, owner_a.pubkey(), outsider.pubkey(), unpause());
    let result = send(&mut svm, &owner_a, &[&outsider], nonmember);
    assert_error_message(result, "Mock multisig signer is not a member");

    let approved =
        mock_execute_instruction(multisig, owner_a.pubkey(), owner_b.pubkey(), unpause());
    send(&mut svm, &owner_a, &[&owner_b], approved).unwrap();
    assert!(!vault_state(&svm, vault).paused);
    assert_eq!(
        vault_state(&svm, vault).namespace_authority,
        developer.pubkey()
    );
}
