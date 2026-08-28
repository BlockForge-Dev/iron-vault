import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";

const programId = new PublicKey("2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1");
const connection = new Connection("http://127.0.0.1:8899", "finalized");
const keypairPath = process.env.ANCHOR_WALLET ?? join(homedir(), ".config", "solana", "id.json");
const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(await readFile(keypairPath, "utf8"))));
const guardian = Keypair.generate().publicKey;
const [protocol] = PublicKey.findProgramAddressSync([Buffer.from("protocol")], programId);

if ((await connection.getBalance(payer.publicKey, "confirmed")) < 1_000_000_000) {
  const airdrop = await connection.requestAirdrop(payer.publicKey, 2_000_000_000);
  await connection.confirmTransaction(airdrop, "finalized");
}

const discriminator = createHash("sha256").update("global:initialize_protocol").digest().subarray(0, 8);
const data = Buffer.concat([discriminator, payer.publicKey.toBuffer(), guardian.toBuffer()]);
const instruction = new TransactionInstruction({
  programId,
  keys: [
    { pubkey: payer.publicKey, isSigner: true, isWritable: true },
    { pubkey: protocol, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ],
  data,
});
const signature = await sendAndConfirmTransaction(connection, new Transaction().add(instruction), [payer], {
  commitment: "finalized",
});
await mkdir("target", { recursive: true });
await writeFile("target/e2e-signature.txt", `${signature}\n`, { mode: 0o600 });
process.stdout.write(`${JSON.stringify({ signature, protocol: protocol.toBase58() })}\n`);
