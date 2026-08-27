import { PublicKey } from "@solana/web3.js";
import { IRON_VAULT_PROGRAM_ADDRESS, PDA_SEEDS } from "./constants.js";
import { encodeU64 } from "./encoding.js";

export function deriveProtocolAddress(programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.protocol)], programId)[0];
}

export function deriveEscrowAddress(maker: PublicKey, escrowId: bigint, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.escrow), maker.toBuffer(), encodeU64(escrowId, "escrowId")], programId)[0];
}

export function deriveEscrowTokenAddress(escrow: PublicKey, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.escrowToken), escrow.toBuffer()], programId)[0];
}

export function deriveVaultAddress(authority: PublicKey, vaultId: bigint, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.vault), authority.toBuffer(), encodeU64(vaultId, "vaultId")], programId)[0];
}

export function deriveVaultAssetAddress(vault: PublicKey, mint: PublicKey, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.vaultAsset), vault.toBuffer(), mint.toBuffer()], programId)[0];
}

export function deriveVaultTokenAddress(vault: PublicKey, mint: PublicKey, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.vaultToken), vault.toBuffer(), mint.toBuffer()], programId)[0];
}

export function deriveRoleAddress(vault: PublicKey, principal: PublicKey, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.role), vault.toBuffer(), principal.toBuffer()], programId)[0];
}

export function deriveWithdrawalAddress(vault: PublicKey, withdrawalId: bigint, programId = IRON_VAULT_PROGRAM_ADDRESS): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(PDA_SEEDS.withdrawal), vault.toBuffer(), encodeU64(withdrawalId, "withdrawalId")], programId)[0];
}
