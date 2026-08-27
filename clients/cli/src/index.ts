#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { pathToFileURL } from "node:url";
import {
  Connection,
  IronVaultClient,
  Keypair,
  keypairFromSecretKey,
} from "@iron-vault/sdk";

export const USAGE = `Usage:
  iron-vault escrow create --recipient <pubkey> --mint <pubkey> --amount <base-units> --expires-at <unix-or-ISO> [--escrow-id <u64>] [--source-token <pubkey>]
  iron-vault escrow release --escrow-id <u64> [--escrow <pubkey>] [--destination-token <pubkey>]
  iron-vault escrow refund --escrow <pubkey> --escrow-id <u64> [--destination-token <pubkey>]
  iron-vault vault create --guardian <pubkey> [--vault-id <u64>]
  iron-vault vault register-asset --vault <pubkey> --mint <pubkey>
  iron-vault vault deposit --vault <pubkey> --mint <pubkey> --amount <base-units> [--source-token <pubkey>]
  iron-vault withdrawal request --vault <pubkey> --mint <pubkey> --recipient <pubkey> --amount <base-units> [--recipient-token <pubkey>]

Global options:
  --url <rpc-url>          Default: http://127.0.0.1:8899
  --keypair <path>        Default: ~/.config/solana/id.json
  --program-id <pubkey>   Override the canonical IronVault program ID
  --json                  Print machine-readable JSON
  --help                  Show this help`;

export interface CliInvocation {
  group: string;
  action: string;
  options: Record<string, string | boolean>;
}

export function parseInvocation(argv: string[]): CliInvocation {
  if (argv.includes("--help") || argv.includes("-h")) return { group: "help", action: "help", options: {} };
  if (argv.length < 2) throw new Error("a command group and action are required");
  const [group, action, ...rest] = argv;
  const options: Record<string, string | boolean> = {};
  for (let index = 0; index < rest.length; index += 1) {
    const flag = rest[index];
    if (!flag?.startsWith("--")) throw new Error(`unexpected argument: ${flag ?? ""}`);
    const name = flag.slice(2);
    if (name === "json") {
      options[name] = true;
      continue;
    }
    const value = rest[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`missing value for --${name}`);
    if (name in options) throw new Error(`duplicate option: --${name}`);
    options[name] = value;
    index += 1;
  }
  return { group: group!, action: action!, options };
}

function required(options: CliInvocation["options"], name: string): string {
  const value = options[name];
  if (typeof value !== "string") throw new Error(`--${name} is required`);
  return value;
}

function optional(options: CliInvocation["options"], name: string): string | undefined {
  const value = options[name];
  return typeof value === "string" ? value : undefined;
}

function integer(value: string, name: string): bigint {
  if (!/^[0-9]+$/.test(value)) throw new Error(`--${name} must be a non-negative integer`);
  return BigInt(value);
}

function expiry(value: string): bigint | Date {
  if (/^-?[0-9]+$/.test(value)) return BigInt(value);
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) throw new Error("--expires-at must be Unix seconds or an ISO date");
  return parsed;
}

function expandHome(path: string): string {
  return path === "~" ? homedir() : path.startsWith("~/") || path.startsWith("~\\")
    ? `${homedir()}${path.slice(1)}` : path;
}

async function loadKeypair(path: string): Promise<Keypair> {
  const raw: unknown = JSON.parse(await readFile(expandHome(path), "utf8"));
  if (!Array.isArray(raw) || !raw.every((value) => Number.isInteger(value) && value >= 0 && value <= 255)) {
    throw new Error("keypair file must contain a JSON byte array");
  }
  return keypairFromSecretKey(raw as number[]);
}

function printable(value: unknown): unknown {
  if (typeof value === "bigint") return value.toString();
  if (value && typeof value === "object" && "toBase58" in value) {
    return (value as { toBase58(): string }).toBase58();
  }
  if (Array.isArray(value)) return value.map(printable);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, nested]) => [key, printable(nested)]));
  }
  return value;
}

export async function run(argv: string[]): Promise<void> {
  const invocation = parseInvocation(argv);
  if (invocation.group === "help") {
    process.stdout.write(`${USAGE}\n`);
    return;
  }
  const options = invocation.options;
  const payer = await loadKeypair(optional(options, "keypair") ?? "~/.config/solana/id.json");
  const client = new IronVaultClient({
    connection: new Connection(optional(options, "url") ?? "http://127.0.0.1:8899", "confirmed"),
    payer,
    programId: optional(options, "program-id"),
  });

  let result: unknown;
  const command = `${invocation.group} ${invocation.action}`;
  switch (command) {
    case "escrow create":
      result = await client.createEscrow({
        recipient: required(options, "recipient"), mint: required(options, "mint"),
        amount: integer(required(options, "amount"), "amount"),
        expiresAt: expiry(required(options, "expires-at")),
        escrowId: optional(options, "escrow-id") ? integer(required(options, "escrow-id"), "escrow-id") : undefined,
        makerToken: optional(options, "source-token"),
      });
      break;
    case "escrow release":
      result = await client.releaseEscrow({
        escrowId: integer(required(options, "escrow-id"), "escrow-id"),
        escrow: optional(options, "escrow"), destinationToken: optional(options, "destination-token"),
      });
      break;
    case "escrow refund":
      result = await client.refundEscrow({
        escrow: required(options, "escrow"), escrowId: integer(required(options, "escrow-id"), "escrow-id"),
        destinationToken: optional(options, "destination-token"),
      });
      break;
    case "vault create":
      result = await client.createVault({
        guardian: required(options, "guardian"),
        vaultId: optional(options, "vault-id") ? integer(required(options, "vault-id"), "vault-id") : undefined,
      });
      break;
    case "vault register-asset":
      result = await client.registerAsset({ vault: required(options, "vault"), mint: required(options, "mint") });
      break;
    case "vault deposit":
      result = await client.deposit({
        vault: required(options, "vault"), mint: required(options, "mint"),
        amount: integer(required(options, "amount"), "amount"), sourceToken: optional(options, "source-token"),
      });
      break;
    case "withdrawal request":
      result = await client.requestWithdrawal({
        vault: required(options, "vault"), mint: required(options, "mint"), recipient: required(options, "recipient"),
        amount: integer(required(options, "amount"), "amount"), recipientToken: optional(options, "recipient-token"),
      });
      break;
    default:
      throw new Error(`unknown command: ${command}`);
  }
  process.stdout.write(`${JSON.stringify(printable(result), null, options.json ? 0 : 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  run(process.argv.slice(2)).catch((error: unknown) => {
    process.stderr.write(`iron-vault: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
