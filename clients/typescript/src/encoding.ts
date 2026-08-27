import { sha256 } from "@noble/hashes/sha256";
import { randomBytes } from "@noble/hashes/utils";
import { PublicKey } from "@solana/web3.js";

const U64_MAX = (1n << 64n) - 1n;
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;

export function anchorDiscriminator(namespace: "global" | "account" | "event", name: string): Buffer {
  return Buffer.from(sha256(new TextEncoder().encode(`${namespace}:${name}`))).subarray(0, 8);
}

export function encodeU64(value: bigint, field = "value"): Buffer {
  if (value < 0n || value > U64_MAX) throw new RangeError(`${field} must fit in u64`);
  const output = Buffer.alloc(8);
  output.writeBigUInt64LE(value);
  return output;
}

export function encodeI64(value: bigint, field = "value"): Buffer {
  if (value < I64_MIN || value > I64_MAX) throw new RangeError(`${field} must fit in i64`);
  const output = Buffer.alloc(8);
  output.writeBigInt64LE(value);
  return output;
}

export function instructionData(name: string, ...fields: Buffer[]): Buffer {
  return Buffer.concat([anchorDiscriminator("global", name), ...fields]);
}

export function randomU64(): bigint {
  return Buffer.from(randomBytes(8)).readBigUInt64LE();
}

export function toBigInt(value: bigint | number | Date, field: string): bigint {
  if (value instanceof Date) return BigInt(Math.floor(value.getTime() / 1_000));
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new RangeError(`${field} must be a safe integer`);
    return BigInt(value);
  }
  return value;
}

export function asPublicKey(value: PublicKey | string): PublicKey {
  return typeof value === "string" ? new PublicKey(value) : value;
}
