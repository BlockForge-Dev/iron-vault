import {
  Commitment,
  Connection,
  Keypair,
  PublicKey,
  SendOptions,
  Signer,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { IRON_VAULT_PROGRAM_ADDRESS } from "./constants.js";
import {
  anchorDiscriminator,
  asPublicKey,
  encodeI64,
  encodeU64,
  instructionData,
  randomU64,
  toBigInt,
} from "./encoding.js";
import {
  deriveEscrowAddress,
  deriveEscrowTokenAddress,
  deriveProtocolAddress,
  deriveRoleAddress,
  deriveVaultAddress,
  deriveVaultAssetAddress,
  deriveVaultTokenAddress,
  deriveWithdrawalAddress,
} from "./pdas.js";
import {
  TOKEN_2022_PROGRAM_ADDRESS,
  TOKEN_PROGRAM_ADDRESS,
  createAssociatedTokenAccountIdempotentInstruction,
  deriveAssociatedTokenAddress,
} from "./token.js";

export type Address = PublicKey | string;

export interface IronVaultClientOptions {
  connection: Connection;
  payer: Signer;
  programId?: Address;
  commitment?: Commitment;
  sendOptions?: SendOptions;
  /** Optional wallet-adapter hook. When omitted, the configured keypair signs directly. */
  transactionSender?: (transaction: Transaction, signers: Signer[]) => Promise<string>;
}

export interface TransactionResult<T> {
  signature: string;
  accounts: T;
}

export interface CreateEscrowParams {
  recipient: Address;
  mint: Address;
  amount: bigint | number;
  expiresAt: bigint | number | Date;
  escrowId?: bigint | number;
  makerToken?: Address;
}

export interface EscrowReference {
  escrowId: bigint | number;
  escrow?: Address;
  destinationToken?: Address;
}

export interface CreateVaultParams {
  guardian: Address;
  vaultId?: bigint | number;
}

export interface VaultMintParams {
  vault: Address;
  mint: Address;
}

export interface DepositParams extends VaultMintParams {
  amount: bigint | number;
  sourceToken?: Address;
}

export interface RequestWithdrawalParams extends VaultMintParams {
  recipient: Address;
  amount: bigint | number;
  recipientToken?: Address;
}

interface EscrowState {
  maker: PublicKey;
  recipient: PublicKey;
  mint: PublicKey;
  tokenProgram: PublicKey;
  escrowId: bigint;
}

interface VaultState {
  authority: PublicKey;
  nextWithdrawalId: bigint;
}

function meta(pubkey: PublicKey, isSigner = false, isWritable = false) {
  return { pubkey, isSigner, isWritable };
}

export class IronVaultClient {
  readonly connection: Connection;
  readonly payer: Signer;
  readonly programId: PublicKey;
  readonly commitment: Commitment;
  readonly sendOptions?: SendOptions;
  readonly transactionSender?: IronVaultClientOptions["transactionSender"];

  constructor(options: IronVaultClientOptions) {
    this.connection = options.connection;
    this.payer = options.payer;
    this.programId = options.programId ? asPublicKey(options.programId) : IRON_VAULT_PROGRAM_ADDRESS;
    this.commitment = options.commitment ?? "confirmed";
    this.sendOptions = options.sendOptions;
    this.transactionSender = options.transactionSender;
  }

  async createEscrow(params: CreateEscrowParams) {
    const maker = this.payer.publicKey;
    const recipient = asPublicKey(params.recipient);
    const mint = asPublicKey(params.mint);
    const amount = toBigInt(params.amount, "amount");
    const expiresAt = toBigInt(params.expiresAt, "expiresAt");
    const escrowId = params.escrowId === undefined ? randomU64() : toBigInt(params.escrowId, "escrowId");
    const tokenProgram = await this.resolveTokenProgram(mint);
    const makerToken = params.makerToken
      ? asPublicKey(params.makerToken)
      : deriveAssociatedTokenAddress(mint, maker, tokenProgram);
    const escrow = deriveEscrowAddress(maker, escrowId, this.programId);
    const escrowToken = deriveEscrowTokenAddress(escrow, this.programId);
    const instruction = new TransactionInstruction({
      programId: this.programId,
      keys: [
        meta(maker, true, true), meta(deriveProtocolAddress(this.programId)), meta(mint),
        meta(makerToken, false, true), meta(escrow, false, true), meta(escrowToken, false, true),
        meta(tokenProgram), meta(SystemProgram.programId),
      ],
      data: instructionData(
        "create_escrow", encodeU64(escrowId, "escrowId"), recipient.toBuffer(),
        encodeU64(amount, "amount"), encodeI64(expiresAt, "expiresAt"),
      ),
    });
    const signature = await this.send([instruction]);
    return { signature, accounts: { escrow, escrowToken, makerToken, tokenProgram, escrowId } };
  }

  async releaseEscrow(params: EscrowReference) {
    const escrowId = toBigInt(params.escrowId, "escrowId");
    const escrow = params.escrow
      ? asPublicKey(params.escrow)
      : deriveEscrowAddress(this.payer.publicKey, escrowId, this.programId);
    const state = await this.fetchEscrow(escrow);
    if (!state.maker.equals(this.payer.publicKey)) throw new Error("connected payer is not the escrow maker");
    const escrowToken = deriveEscrowTokenAddress(escrow, this.programId);
    const recipientToken = params.destinationToken
      ? asPublicKey(params.destinationToken)
      : deriveAssociatedTokenAddress(state.mint, state.recipient, state.tokenProgram);
    const instructions = params.destinationToken ? [] : [
      createAssociatedTokenAccountIdempotentInstruction(
        this.payer.publicKey, recipientToken, state.recipient, state.mint,
        state.tokenProgram,
      ),
    ];
    instructions.push(new TransactionInstruction({
      programId: this.programId,
      keys: [
        meta(this.payer.publicKey, true), meta(deriveProtocolAddress(this.programId)),
        meta(escrow, false, true), meta(state.mint), meta(escrowToken, false, true),
        meta(recipientToken, false, true), meta(state.tokenProgram),
      ],
      data: instructionData("release_escrow"),
    }));
    const signature = await this.send(instructions);
    return { signature, accounts: { escrow, escrowToken, recipientToken } };
  }

  async refundEscrow(params: EscrowReference) {
    const escrowId = toBigInt(params.escrowId, "escrowId");
    if (!params.escrow) throw new Error("refund requires the escrow address when caller may differ from maker");
    const escrow = asPublicKey(params.escrow);
    const state = await this.fetchEscrow(escrow);
    if (state.escrowId !== escrowId) throw new Error("escrowId does not match escrow state");
    const escrowToken = deriveEscrowTokenAddress(escrow, this.programId);
    const makerDestination = params.destinationToken
      ? asPublicKey(params.destinationToken)
      : deriveAssociatedTokenAddress(state.mint, state.maker, state.tokenProgram);
    const instructions = params.destinationToken ? [] : [
      createAssociatedTokenAccountIdempotentInstruction(
        this.payer.publicKey, makerDestination, state.maker, state.mint,
        state.tokenProgram,
      ),
    ];
    instructions.push(new TransactionInstruction({
      programId: this.programId,
      keys: [
        meta(this.payer.publicKey, true), meta(escrow, false, true), meta(state.mint),
        meta(escrowToken, false, true), meta(makerDestination, false, true),
        meta(state.tokenProgram), meta(SYSVAR_CLOCK_PUBKEY),
      ],
      data: instructionData("refund_escrow"),
    }));
    const signature = await this.send(instructions);
    return { signature, accounts: { escrow, escrowToken, makerDestination } };
  }

  async createVault(params: CreateVaultParams) {
    const guardian = asPublicKey(params.guardian);
    const vaultId = params.vaultId === undefined ? randomU64() : toBigInt(params.vaultId, "vaultId");
    const vault = deriveVaultAddress(this.payer.publicKey, vaultId, this.programId);
    const instruction = new TransactionInstruction({
      programId: this.programId,
      keys: [meta(this.payer.publicKey, true, true), meta(deriveProtocolAddress(this.programId)), meta(vault, false, true), meta(SystemProgram.programId)],
      data: instructionData("create_vault", encodeU64(vaultId, "vaultId"), guardian.toBuffer()),
    });
    const signature = await this.send([instruction]);
    return { signature, accounts: { vault, vaultId } };
  }

  async registerAsset(params: VaultMintParams) {
    const vault = asPublicKey(params.vault);
    const mint = asPublicKey(params.mint);
    const tokenProgram = await this.resolveTokenProgram(mint);
    const vaultAsset = deriveVaultAssetAddress(vault, mint, this.programId);
    const vaultToken = deriveVaultTokenAddress(vault, mint, this.programId);
    const instruction = new TransactionInstruction({
      programId: this.programId,
      keys: [
        meta(this.payer.publicKey, true, true), meta(deriveProtocolAddress(this.programId)), meta(vault),
        meta(mint), meta(vaultAsset, false, true), meta(vaultToken, false, true),
        meta(tokenProgram), meta(SystemProgram.programId),
      ],
      data: instructionData("register_asset"),
    });
    const signature = await this.send([instruction]);
    return { signature, accounts: { vault, vaultAsset, vaultToken, tokenProgram } };
  }

  async deposit(params: DepositParams) {
    const vault = asPublicKey(params.vault);
    const mint = asPublicKey(params.mint);
    const amount = toBigInt(params.amount, "amount");
    const tokenProgram = await this.resolveTokenProgram(mint);
    const sourceToken = params.sourceToken
      ? asPublicKey(params.sourceToken)
      : deriveAssociatedTokenAddress(mint, this.payer.publicKey, tokenProgram);
    const vaultAsset = deriveVaultAssetAddress(vault, mint, this.programId);
    const vaultToken = deriveVaultTokenAddress(vault, mint, this.programId);
    const instruction = new TransactionInstruction({
      programId: this.programId,
      keys: [
        meta(this.payer.publicKey, true), meta(vault), meta(mint), meta(vaultAsset),
        meta(sourceToken, false, true), meta(vaultToken, false, true), meta(tokenProgram),
      ],
      data: instructionData("deposit", encodeU64(amount, "amount")),
    });
    const signature = await this.send([instruction]);
    return { signature, accounts: { vault, vaultAsset, vaultToken, sourceToken } };
  }

  async requestWithdrawal(params: RequestWithdrawalParams) {
    const vault = asPublicKey(params.vault);
    const mint = asPublicKey(params.mint);
    const recipient = asPublicKey(params.recipient);
    const amount = toBigInt(params.amount, "amount");
    const tokenProgram = await this.resolveTokenProgram(mint);
    const vaultState = await this.fetchVault(vault);
    const recipientToken = params.recipientToken
      ? asPublicKey(params.recipientToken)
      : deriveAssociatedTokenAddress(mint, recipient, tokenProgram);
    const vaultAsset = deriveVaultAssetAddress(vault, mint, this.programId);
    const withdrawalRequest = deriveWithdrawalAddress(vault, vaultState.nextWithdrawalId, this.programId);
    const keys = [
      meta(this.payer.publicKey, true, true), meta(deriveProtocolAddress(this.programId)),
      meta(vault, false, true), meta(mint), meta(vaultAsset), meta(recipientToken),
      meta(withdrawalRequest, false, true), meta(SystemProgram.programId), meta(SYSVAR_CLOCK_PUBKEY),
    ];
    if (!vaultState.authority.equals(this.payer.publicKey)) {
      keys.push(meta(deriveRoleAddress(vault, this.payer.publicKey, this.programId)));
    }
    const instructions = params.recipientToken ? [] : [
      createAssociatedTokenAccountIdempotentInstruction(
        this.payer.publicKey, recipientToken, recipient, mint, tokenProgram,
      ),
    ];
    instructions.push(new TransactionInstruction({
      programId: this.programId,
      keys,
      data: instructionData("request_withdrawal", encodeU64(amount, "amount")),
    }));
    const signature = await this.send(instructions);
    return { signature, accounts: { vault, vaultAsset, withdrawalRequest, recipientToken, withdrawalId: vaultState.nextWithdrawalId } };
  }

  private async resolveTokenProgram(mint: PublicKey): Promise<PublicKey> {
    const account = await this.connection.getAccountInfo(mint, this.commitment);
    if (!account) throw new Error(`mint account not found: ${mint.toBase58()}`);
    if (!account.owner.equals(TOKEN_PROGRAM_ADDRESS) && !account.owner.equals(TOKEN_2022_PROGRAM_ADDRESS)) {
      throw new Error(`unsupported mint owner: ${account.owner.toBase58()}`);
    }
    return account.owner;
  }

  private async fetchEscrow(address: PublicKey): Promise<EscrowState> {
    const data = await this.fetchProgramAccount(address, "Escrow", 170);
    return {
      maker: new PublicKey(data.subarray(8, 40)), recipient: new PublicKey(data.subarray(40, 72)),
      mint: new PublicKey(data.subarray(72, 104)), tokenProgram: new PublicKey(data.subarray(104, 136)),
      escrowId: data.readBigUInt64LE(136),
    };
  }

  private async fetchVault(address: PublicKey): Promise<VaultState> {
    const data = await this.fetchProgramAccount(address, "Vault", 120);
    return { authority: new PublicKey(data.subarray(40, 72)), nextWithdrawalId: data.readBigUInt64LE(112) };
  }

  private async fetchProgramAccount(address: PublicKey, type: string, minimumLength: number): Promise<Buffer> {
    const account = await this.connection.getAccountInfo(address, this.commitment);
    if (!account) throw new Error(`${type} account not found: ${address.toBase58()}`);
    if (!account.owner.equals(this.programId)) throw new Error(`${type} account has the wrong owner`);
    const data = Buffer.from(account.data);
    if (data.length < minimumLength || !data.subarray(0, 8).equals(anchorDiscriminator("account", type))) {
      throw new Error(`${type} account data is invalid`);
    }
    return data;
  }

  private async send(instructions: TransactionInstruction[]): Promise<string> {
    const transaction = new Transaction().add(...instructions);
    if (this.transactionSender) return this.transactionSender(transaction, [this.payer]);
    return sendAndConfirmTransaction(this.connection, transaction, [this.payer], {
      commitment: this.commitment,
      ...this.sendOptions,
    });
  }
}

export function keypairFromSecretKey(bytes: number[] | Uint8Array): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(bytes));
}
