import { PublicKey } from "@solana/web3.js";

export const IRON_VAULT_PROGRAM_ID =
  "2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1" as const;
export const IRON_VAULT_PROGRAM_ADDRESS = new PublicKey(IRON_VAULT_PROGRAM_ID);

export const TOKEN_PROGRAM_IDS = Object.freeze({
  legacy: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
  token2022: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
} as const);

export const TOKEN_2022_POLICY = Object.freeze({ allowMintExtensions: false } as const);

export const PDA_SEEDS = Object.freeze({
  protocol: "protocol",
  escrow: "escrow",
  escrowToken: "escrow_token",
  vault: "vault",
  vaultAsset: "vault_asset",
  vaultToken: "vault_token",
  role: "role",
  withdrawal: "withdrawal",
} as const);
export type PdaSeedName = keyof typeof PDA_SEEDS;

export const VAULT_PERMISSIONS = Object.freeze({
  withdraw: 1n << 0n,
  requestWithdrawal: 1n << 1n,
  cancelWithdrawal: 1n << 2n,
  manageAssets: 1n << 3n,
  manageLimits: 1n << 4n,
  manageRoles: 1n << 5n,
} as const);
export type VaultPermissionName = keyof typeof VAULT_PERMISSIONS;

export const PROTOCOL_PAUSE_FLAGS = Object.freeze({
  escrowCreate: 1 << 0,
  escrowRelease: 1 << 1,
  vaultConfig: 1 << 2,
  vaultOutflow: 1 << 3,
} as const);
export type ProtocolPauseFlagName = keyof typeof PROTOCOL_PAUSE_FLAGS;

export interface VaultWithdrawalPolicy {
  maxPerTransaction: bigint;
  windowLimit: bigint;
  windowSeconds: bigint;
  timelockThreshold: bigint;
  timelockSeconds: bigint;
  requestExecutionWindowSeconds: bigint;
}
