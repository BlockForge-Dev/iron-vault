# Events, TypeScript SDK, and CLI

## Event contract

Every instruction that creates or mutates protocol state emits one Anchor event.
The no-op compatibility `initialize` instruction creates no state and emits no
event. Every event payload starts with the shared `u16` schema version after its
8-byte Anchor discriminator.

| Transition | On-chain event |
|---|---|
| Protocol initialized | `ProtocolInitialized` |
| Protocol pause mask changed | `ProtocolPauseUpdated` |
| Escrow funded | `EscrowCreated` |
| Escrow released | `EscrowReleased` |
| Escrow refunded | `EscrowRefunded` |
| Vault created | `VaultCreated` |
| Vault authority changed | `VaultAuthorityUpdated` |
| Vault paused or unpaused | `VaultPauseUpdated` (`paused` identifies direction) |
| Asset registered | `VaultAssetRegistered` |
| Deposit received | `VaultDeposit` |
| Instant withdrawal executed | `VaultWithdrawal` |
| Role granted/replaced | `RoleGranted` |
| Role revoked | `RoleRevoked` |
| Withdrawal policy changed | `VaultLimitsUpdated` |
| Timelocked withdrawal requested | `WithdrawalRequested` |
| Timelocked withdrawal executed | `WithdrawalExecuted` |
| Timelocked withdrawal cancelled | `WithdrawalCancelled` |

The SDK exports `IRON_VAULT_EVENT_NAMES`, exact discriminator values,
`identifyIronVaultEvent`, `decodeIronVaultEventEnvelope`, and
`decodeIronVaultEventLog`. The envelope decoder returns the event identity,
schema version, and remaining Borsh payload. Events are observability data:
indexers must tolerate duplicates and reconcile against finalized account state.

## SDK setup

Construct the client with an RPC connection and signer:

```ts
import { Connection, IronVaultClient, Keypair } from "@iron-vault/sdk";

const ironVault = new IronVaultClient({
  connection: new Connection("http://127.0.0.1:8899", "confirmed"),
  payer: Keypair.fromSecretKey(secretKey),
});

const result = await ironVault.createEscrow({
  recipient,
  mint,
  amount: 1_000_000n,
  expiresAt: new Date("2030-01-01T00:00:00Z"),
});

console.log(result.signature, result.accounts.escrow.toBase58());
```

`escrowId` and `vaultId` default to cryptographically random `u64` values and
are returned with the derived accounts. Callers may supply an explicit ID. The
client derives PDAs, selects the token program from the mint owner, derives token
accounts, creates missing canonical destination ATAs idempotently, and fetches
escrow/vault state when immutable terms or the next request ID are required. A
`transactionSender` hook supports wallet-adapter integrations.

Available transaction methods are `createEscrow`, `releaseEscrow`,
`refundEscrow`, `createVault`, `registerAsset`, `deposit`, and
`requestWithdrawal`. Amounts are raw token base units. The SDK does not infer UI
decimal amounts, create source token accounts, mint tokens, or bypass on-chain
authorization and policy checks.

## CLI

Build the workspace and display help:

```bash
pnpm build
pnpm iron-vault --help
```

Examples:

```bash
pnpm iron-vault escrow create \
  --recipient <RECIPIENT> --mint <MINT> --amount 1000000 \
  --expires-at 2030-01-01T00:00:00Z

pnpm iron-vault escrow release --escrow-id <ID>
pnpm iron-vault escrow refund --escrow <ESCROW_PDA> --escrow-id <ID>

pnpm iron-vault vault create --guardian <GUARDIAN>
pnpm iron-vault vault register-asset --vault <VAULT_PDA> --mint <MINT>
pnpm iron-vault vault deposit --vault <VAULT_PDA> --mint <MINT> --amount 1000000

pnpm iron-vault withdrawal request \
  --vault <VAULT_PDA> --mint <MINT> --recipient <RECIPIENT> --amount 50000000
```

Global options select `--url`, `--keypair`, `--program-id`, and `--json`.
Defaults are localnet and `~/.config/solana/id.json`. The CLI never prints the
secret key. Success output contains the transaction signature and derived public
accounts. The configured RPC commitment is `confirmed`; production automation
must perform finalized reconciliation before treating an event or returned
signature as irreversible.
