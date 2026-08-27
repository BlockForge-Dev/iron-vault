import assert from "node:assert/strict";
import test from "node:test";

import {
  anchorDiscriminator,
  deriveEscrowAddress,
  deriveEscrowTokenAddress,
  deriveAssociatedTokenAddress,
  deriveProtocolAddress,
  decodeIronVaultEventEnvelope,
  identifyIronVaultEvent,
  IRON_VAULT_PROGRAM_ID,
  IRON_VAULT_PROGRAM_ADDRESS,
  IronVaultClient,
  Keypair,
  PDA_SEEDS,
  PublicKey,
  PROTOCOL_PAUSE_FLAGS,
  TOKEN_2022_POLICY,
  TOKEN_PROGRAM_ADDRESS,
  TOKEN_PROGRAM_IDS,
  VAULT_PERMISSIONS,
} from "../dist/index.js";

test("exports the canonical program address", () => {
  assert.equal(
    IRON_VAULT_PROGRAM_ID,
    "2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1",
  );
});

test("exports the exact fail-closed token policy", () => {
  assert.deepEqual(TOKEN_PROGRAM_IDS, {
    legacy: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    token2022: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
  });
  assert.deepEqual(TOKEN_2022_POLICY, { allowMintExtensions: false });
  assert.ok(Object.isFrozen(TOKEN_PROGRAM_IDS));
  assert.ok(Object.isFrozen(TOKEN_2022_POLICY));
});

test("exports every specified PDA namespace", () => {
  assert.deepEqual(Object.values(PDA_SEEDS), [
    "protocol",
    "escrow",
    "escrow_token",
    "vault",
    "vault_asset",
    "vault_token",
    "role",
    "withdrawal",
  ]);
  assert.ok(Object.isFrozen(PDA_SEEDS));
});

test("exports the exact reserved vault permission bits", () => {
  assert.deepEqual(Object.values(VAULT_PERMISSIONS), [1n, 2n, 4n, 8n, 16n, 32n]);
  assert.ok(Object.isFrozen(VAULT_PERMISSIONS));
});

test("exports the exact directional protocol pause bits", () => {
  assert.deepEqual(Object.values(PROTOCOL_PAUSE_FLAGS), [1, 2, 4, 8]);
  assert.ok(Object.isFrozen(PROTOCOL_PAUSE_FLAGS));
});

test("derives stable escrow and custody PDAs", () => {
  const maker = new PublicKey("11111111111111111111111111111111");
  const escrow = deriveEscrowAddress(maker, 7n);
  assert.equal(escrow.toBase58(), deriveEscrowAddress(maker, 7n).toBase58());
  assert.notEqual(escrow.toBase58(), deriveEscrowAddress(maker, 8n).toBase58());
  assert.equal(deriveEscrowTokenAddress(escrow).toBuffer().length, 32);
  assert.equal(deriveProtocolAddress().toBuffer().length, 32);
});

test("matches the pinned SPL CLI associated-token regression vector", () => {
  const owner = new PublicKey("11111111111111111111111111111111");
  const mint = new PublicKey("So11111111111111111111111111111111111111112");
  assert.equal(
    deriveAssociatedTokenAddress(mint, owner, TOKEN_PROGRAM_ADDRESS).toBase58(),
    "aqxoAhCwpy3oB1BpNw9hL1HdLYLgPpbPjzxDrrQj3Fs",
  );
});

test("identifies all emitted Anchor event discriminators", () => {
  const version = Buffer.from([1, 0]);
  const encoded = Buffer.concat([anchorDiscriminator("event", "EscrowCreated"), version, Buffer.alloc(10)]);
  assert.equal(identifyIronVaultEvent(encoded), "EscrowCreated");
  assert.deepEqual(decodeIronVaultEventEnvelope(encoded), {
    name: "EscrowCreated",
    version: 1,
    payload: Buffer.alloc(10),
  });
  assert.equal(identifyIronVaultEvent(Buffer.alloc(8)), undefined);
});

test("createEscrow derives accounts and encodes the Anchor instruction", async () => {
  const payer = Keypair.generate();
  const mint = Keypair.generate().publicKey;
  const recipient = Keypair.generate().publicKey;
  let captured;
  const connection = {
    getAccountInfo: async (address) => address.equals(mint)
      ? { owner: TOKEN_PROGRAM_ADDRESS, data: Buffer.alloc(82) }
      : null,
  };
  const client = new IronVaultClient({
    connection,
    payer,
    transactionSender: async (transaction, signers) => {
      captured = { transaction, signers };
      return "test-signature";
    },
  });
  const result = await client.createEscrow({
    recipient,
    mint,
    amount: 25n,
    expiresAt: 2_000_000_000n,
    escrowId: 42n,
  });

  assert.equal(result.signature, "test-signature");
  assert.equal(result.accounts.escrow.toBase58(), deriveEscrowAddress(payer.publicKey, 42n).toBase58());
  assert.equal(captured.signers[0], payer);
  const instruction = captured.transaction.instructions[0];
  assert.equal(instruction.keys.length, 8);
  assert.equal(instruction.keys[0].isSigner, true);
  assert.equal(instruction.data.subarray(0, 8).toString("hex"), anchorDiscriminator("global", "create_escrow").toString("hex"));
  assert.equal(instruction.data.readBigUInt64LE(8), 42n);
  assert.equal(instruction.data.readBigUInt64LE(48), 25n);
  assert.equal(instruction.data.readBigInt64LE(56), 2_000_000_000n);
});

test("all documented transaction methods encode their matching Anchor entrypoints", async () => {
  const payer = Keypair.generate();
  const recipient = Keypair.generate().publicKey;
  const guardian = Keypair.generate().publicKey;
  const mint = Keypair.generate().publicKey;
  const vault = Keypair.generate().publicKey;
  const escrow = deriveEscrowAddress(payer.publicKey, 7n);
  const destinationToken = Keypair.generate().publicKey;
  const accountData = new Map();

  const vaultData = Buffer.alloc(168);
  anchorDiscriminator("account", "Vault").copy(vaultData);
  payer.publicKey.toBuffer().copy(vaultData, 40);
  vaultData.writeBigUInt64LE(3n, 112);
  accountData.set(vault.toBase58(), { owner: IRON_VAULT_PROGRAM_ADDRESS, data: vaultData });

  const escrowData = Buffer.alloc(200);
  anchorDiscriminator("account", "Escrow").copy(escrowData);
  payer.publicKey.toBuffer().copy(escrowData, 8);
  recipient.toBuffer().copy(escrowData, 40);
  mint.toBuffer().copy(escrowData, 72);
  TOKEN_PROGRAM_ADDRESS.toBuffer().copy(escrowData, 104);
  escrowData.writeBigUInt64LE(7n, 136);
  accountData.set(escrow.toBase58(), { owner: IRON_VAULT_PROGRAM_ADDRESS, data: escrowData });
  accountData.set(mint.toBase58(), { owner: TOKEN_PROGRAM_ADDRESS, data: Buffer.alloc(82) });

  const captured = [];
  const client = new IronVaultClient({
    connection: { getAccountInfo: async (address) => accountData.get(address.toBase58()) ?? null },
    payer,
    transactionSender: async (transaction) => {
      captured.push(transaction);
      return `signature-${captured.length}`;
    },
  });

  await client.releaseEscrow({ escrowId: 7n, destinationToken });
  await client.refundEscrow({ escrowId: 7n, escrow, destinationToken });
  await client.createVault({ guardian, vaultId: 9n });
  await client.registerAsset({ vault, mint });
  await client.deposit({ vault, mint, amount: 11n, sourceToken: destinationToken });
  const requested = await client.requestWithdrawal({
    vault, mint, recipient, amount: 12n, recipientToken: destinationToken,
  });

  const expected = [
    ["release_escrow", 7], ["refund_escrow", 7], ["create_vault", 4],
    ["register_asset", 8], ["deposit", 7], ["request_withdrawal", 9],
  ];
  for (const [index, [name, accountCount]] of expected.entries()) {
    const instruction = captured[index].instructions.at(-1);
    assert.equal(instruction.data.subarray(0, 8).toString("hex"), anchorDiscriminator("global", name).toString("hex"));
    assert.equal(instruction.keys.length, accountCount);
  }
  assert.equal(requested.accounts.withdrawalId, 3n);
});
