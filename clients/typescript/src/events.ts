import { anchorDiscriminator } from "./encoding.js";

export const IRON_VAULT_EVENT_NAMES = Object.freeze([
  "ProtocolInitialized", "ProtocolPauseUpdated", "EscrowCreated", "EscrowReleased",
  "EscrowRefunded", "VaultCreated", "VaultAuthorityUpdated", "VaultPauseUpdated",
  "VaultAssetRegistered", "VaultDeposit", "VaultWithdrawal", "RoleGranted", "RoleRevoked",
  "VaultLimitsUpdated", "WithdrawalRequested", "WithdrawalExecuted", "WithdrawalCancelled",
] as const);
export type IronVaultEventName = (typeof IRON_VAULT_EVENT_NAMES)[number];

export const IRON_VAULT_EVENT_DISCRIMINATORS = Object.freeze(Object.fromEntries(
  IRON_VAULT_EVENT_NAMES.map((name) => [name, anchorDiscriminator("event", name).toString("hex")]),
) as Record<IronVaultEventName, string>);

export function identifyIronVaultEvent(data: Buffer | Uint8Array): IronVaultEventName | undefined {
  const discriminator = Buffer.from(data).subarray(0, 8).toString("hex");
  return IRON_VAULT_EVENT_NAMES.find((name) => IRON_VAULT_EVENT_DISCRIMINATORS[name] === discriminator);
}

export interface IronVaultEventEnvelope {
  name: IronVaultEventName;
  version: number;
  /** Borsh event fields after the shared u16 schema version. */
  payload: Buffer;
}

export function decodeIronVaultEventEnvelope(data: Buffer | Uint8Array): IronVaultEventEnvelope {
  const encoded = Buffer.from(data);
  if (encoded.length < 10) throw new Error("IronVault event data is truncated");
  const name = identifyIronVaultEvent(encoded);
  if (!name) throw new Error("unknown IronVault event discriminator");
  return { name, version: encoded.readUInt16LE(8), payload: encoded.subarray(10) };
}

/** Decodes one Anchor `Program data:` log without treating logs as authoritative state. */
export function decodeIronVaultEventLog(log: string): IronVaultEventEnvelope | undefined {
  const prefix = "Program data: ";
  if (!log.startsWith(prefix)) return undefined;
  return decodeIronVaultEventEnvelope(Buffer.from(log.slice(prefix.length), "base64"));
}
