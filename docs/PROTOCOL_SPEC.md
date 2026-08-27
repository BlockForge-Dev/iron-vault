# IronVault Protocol Specification

Status: Milestone 0 design baseline
Version: 0.1.0-draft

## 1. Scope

IronVault has two custody products:

1. **Escrow** holds one fixed amount of one token mint. The maker may release it
   to a recipient before expiry; at or after expiry anyone may cause it to be
   refunded to the maker's fixed token destination.
2. **Secure vault** holds one or more separately governed token assets. Roles,
   pauses, per-transaction limits, rolling-window limits, and timelocks constrain
   outflows.

Fees, arbitrary cancellation, partial escrow release, recipient replacement,
admin rescue, oracle-priced limits, native SOL custody, confidential transfers,
and custom multisig logic are out of scope for v1.

## 2. Terms and actors

- **Protocol admin**: changes protocol administration and global pause flags; it
  has no token-withdrawal power.
- **Protocol guardian**: may set global pause flags; it cannot clear them or move
  tokens.
- **Maker**: creates and funds an escrow and may release it.
- **Recipient**: immutable beneficiary of a released escrow.
- **Vault authority**: configures one vault and its roles. It is expected to be
  an external multisig-controlled signer in production.
- **Vault guardian**: may pause its vault and cancel pending withdrawals; it
  cannot transfer tokens or unpause.
- **Operator**: principal with permissions recorded in a role assignment.
- **Cranker**: any caller that executes a permissionless refund or matured
  withdrawal without choosing or changing its destination.

External multisig and program upgrade authority are deployment concerns. The
program accepts ordinary Solana signers and does not implement a multisig.

## 3. Clock and arithmetic

All times are Unix seconds read from the Solana Clock sysvar. Expiry and
execution comparisons are inclusive: `now >= boundary` means the boundary has
been reached. Client-provided time is never authoritative.

All token amounts are raw integer base units. Checked arithmetic is mandatory.
Zero amounts, negative durations, overflow, and invalid time ordering MUST be
rejected. The program makes no price or decimal-normalized comparisons.

## 4. Global invariants

- **G1 — Program binding:** Every mint, token account, and stored token-program
  key in one operation MUST refer to the same supported token program.
- **G2 — Exact custody:** Custody token accounts are PDA-authority accounts for
  their corresponding IronVault state. They MUST have the specified mint, token
  program, and authority and MUST NOT use a caller-selected delegate or close
  authority.
- **G3 — No administrative drain:** Protocol admin and guardians receive no
  instruction that can redirect or withdraw custodial tokens.
- **G4 — Atomic effects:** A failed token CPI or failed state check leaves no
  state mutation. Token CPI, accounting, terminal transition, and event emission
  occur in one Solana transaction.
- **G5 — Explicit destinations:** Every outflow destination's token program,
  mint, and owner MUST be checked against immutable state or authorized request
  data.
- **G6 — Token semantics allowlist:** Unsupported mint/account extensions MUST be
  rejected before deposits or escrow funding. Original SPL Token is permitted.
  Token-2022 is permitted only when its initialized mint-extension list is
  empty. Every extension is denied by default until explicitly reviewed,
  implemented, specified, and tested.
- **G7 — Upgrade separation:** Operational protocol/vault keys MUST NOT be the
  deployed program's upgrade authority. This is an operational invariant that
  cannot be enforced by the program itself.

## 5. Escrow rules

- **E1:** Custodied escrow funds leave only for the immutable recipient through
  release or the immutable maker through refund.
- **E2:** Neither protocol admin nor any guardian can redirect escrow funds.
- **E3:** Exactly one terminal transition is possible: `Funded -> Released` or
  `Funded -> Refunded`.
- **E4:** Maker, recipient, mint, token program, amount, and expiry are immutable.
- Creation and funding are atomic. An unfunded escrow state cannot be created.
- `expires_at` MUST be strictly later than creation time.
- Release is allowed only while `now < expires_at`.
- Refund is permissionless only when `now >= expires_at`; the caller cannot
  select the beneficiary, only supply a token account owned by the maker.
- Terminal escrow state remains as an on-chain tombstone in v1. The empty
  escrow token account MAY be closed only to the maker after the terminal state
  has been written; the escrow state account MUST NOT be closed.
- Protocol pause may stop new escrow creation and pre-expiry releases. It MUST
  NOT disable an expired refund.

## 6. Vault rules

- **V1:** Only the authority or principals with the required active permission
  may initiate outflows.
- **V2:** A guardian may stop dangerous activity and cancel a pending request,
  but cannot withdraw funds or unpause.
- **V3:** An amount greater than `timelock_threshold` cannot use instant
  withdrawal. Equality is instant-eligible, subject to all other limits.
- **V4:** A request's vault, asset, recipient owner, recipient token account,
  amount, creation time, execution time, expiry, and proposer are immutable.
- **V5:** A request executes at most once and remains as a terminal tombstone.
- **V6:** Upgrade authority is distinct from ordinary operational keys.
- Every asset has independent policy and rolling-window accounting.
- Deposits are permissionless when the asset is enabled; pause does not block
  deposits. Deposit amount MUST be positive.
- New instant withdrawals and new withdrawal requests are blocked while either
  the applicable protocol or vault pause is set.
- Execution is also blocked while paused, allowing a guardian to intervene
  during a timelock. Cancellation remains available while paused.
- A local vault pause does not block authority/configuration instructions. This
  lets the authority rotate a compromised guardian, revoke roles, or tighten
  policy before unpausing. The protocol-level `VAULT_CONFIG` flag does block
  vault creation and configuration until the protocol admin clears it.
- Disabled assets reject deposits and all new outflows. Existing pending
  requests may be cancelled but not executed until re-enabled.
- A vault authority cannot use configuration changes to mutate an existing
  request. A request is evaluated against its immutable amount and times, but
  execution also applies the current enabled/pause state and current
  per-transaction/window limits. Tightening policy therefore takes effect
  immediately; loosening policy does not shorten the recorded timelock.

### 6.1 Permissions

The v1 role bitmask reserves these bits:

| Bit | Permission | Capability |
|---:|---|---|
| 0 | `WITHDRAW` | Perform an eligible instant withdrawal |
| 1 | `REQUEST_WITHDRAWAL` | Create a timelocked withdrawal request |
| 2 | `CANCEL_WITHDRAWAL` | Cancel a pending request |
| 3 | `MANAGE_ASSETS` | Register or enable/disable an asset |
| 4 | `MANAGE_LIMITS` | Change an existing asset policy |
| 5 | `MANAGE_ROLES` | Grant, replace, or revoke non-authority roles |

Unknown bits MUST be zero. The vault authority has all configuration powers
without a `RoleAssignment`. A role cannot grant authority-only operations
(`set_authority`, `set_guardian`, or `unpause`). A role manager cannot grant or
revoke permissions it does not itself hold: for a non-authority caller, both the
old and new masks MUST be subsets of the caller's current mask. This prevents a
principal holding only `MANAGE_ROLES` from granting itself `WITHDRAW`. A
principal cannot exercise a revoked role merely because the role account still
exists; its active flag and permissions are checked on every call.

### 6.2 Pause scopes

| Pause | Set by | Cleared by | Blocked operations | Explicitly still allowed |
|---|---|---|---|---|
| `ESCROW_CREATE` | Protocol admin or guardian | Protocol admin | `create_escrow` | release, expired refund |
| `ESCROW_RELEASE` | Protocol admin or guardian | Protocol admin | `release_escrow` | expired refund |
| `VAULT_CONFIG` | Protocol admin or guardian | Protocol admin | create vault; vault/asset/role configuration | deposits, cancellation, local pause |
| `VAULT_OUTFLOW` | Protocol admin or guardian | Protocol admin | instant withdrawal, new request, request execution | deposits, cancellation, local pause/configuration |
| Local vault pause | Vault authority or guardian | Vault authority | instant withdrawal, new request, request execution | deposits, cancellation, authority/configuration operations |

Protocol key rotation and protocol pause management are never blocked by pause
flags. `paused config` below means the protocol-level `VAULT_CONFIG` bit; local
vault pause has the narrower scope shown above.

### 6.3 Limits and timelocks

For an attempted outflow of `amount`:

1. Require `amount > 0` and `amount <= max_per_transaction`.
2. If the stored window has elapsed (`now >= window_started_at +
   window_seconds`), reset its start to `now` and spent amount to zero.
3. Require `window_spent + amount <= window_limit` using checked arithmetic.
4. For instant withdrawal, require `amount <= timelock_threshold`.
5. For a request, require `amount > timelock_threshold`; set
   `execute_after = created_at + timelock_seconds` and `expires_at =
   execute_after + request_execution_window_seconds`.
6. Charge `window_spent` only when tokens actually leave custody, never when a
   request is created or cancelled.

`window_seconds`, `timelock_seconds`, and
`request_execution_window_seconds` MUST be positive. The maximum transaction
and threshold MUST not exceed the window limit. A zero threshold means every
positive withdrawal requires a timelock.

## 7. Protocol instructions

This table is normative. “Changes” includes token movement. All omitted
preconditions include canonical PDA, ownership, serialization discriminator,
supported token-program, signer, checked-arithmetic, and non-duplicate mutable
account checks.

| Instruction | Who may call | Required state / rejection conditions | Changes and token flow | Result |
|---|---|---|---|---|
| `initialize_protocol(admin, guardian)` | Deployment initializer signer; protocol PDA must not exist | Reject zero/default or equal admin and guardian keys | Creates `ProtocolConfig`; no tokens | Active protocol v1 |
| `set_protocol_pause(flags)` | Protocol admin may set/clear; protocol guardian may only add bits | Reject unknown bits or guardian clearing any bit | Updates pause flags; emits event | Requested scopes paused/unpaused |
| `set_protocol_admin(new_admin)` | Current protocol admin | Reject default, unchanged, or guardian key | Replaces admin; no tokens | New admin effective immediately |
| `set_protocol_guardian(new_guardian)` | Protocol admin | Reject default, unchanged, or admin key | Replaces guardian; no tokens | New guardian effective immediately |
| `create_escrow(id, recipient, amount, expires_at)` | Maker signer and source-token owner | Reject paused creation, reused `(maker,id)`, self/default recipient, zero amount, non-future expiry, unsupported token semantics, source mismatch/insufficient funds | Creates escrow and custody token PDA; transfers exactly `amount` source -> custody; emits event | `Funded` escrow with immutable terms |
| `release_escrow()` | Stored maker signer | Reject paused release, non-`Funded`, `now >= expires_at`, recipient destination mismatch, custody balance below amount | Transfers exactly amount custody -> token account owned by stored recipient; sets `Released`; emits event | Empty custody and terminal tombstone |
| `refund_escrow()` | Anyone | Reject non-`Funded`, `now < expires_at`, maker destination mismatch, custody balance below amount | Transfers exactly amount custody -> token account owned by stored maker; sets `Refunded`; emits event | Empty custody and terminal tombstone |
| `create_vault(id, guardian)` | Authority signer | Reject paused config, reused `(authority,id)`, default/equal guardian | Creates vault; no tokens | Active, unpaused vault |
| `set_vault_authority(new_authority)` | Current authority signer | Reject paused config, default, unchanged, or guardian key; canonical vault address remains unchanged and is keyed by original namespace authority | Replaces stored authority; no tokens | New authority effective immediately |
| `set_vault_guardian(new_guardian)` | Authority signer | Reject paused config, default, unchanged, or authority key | Replaces guardian; no tokens | New guardian effective immediately |
| `pause_vault()` | Stored authority or guardian signer | Reject already paused | Sets `paused=true`; no tokens | Outflows and new requests stopped |
| `unpause_vault()` | Stored authority signer only | Reject not paused | Sets `paused=false`; no tokens | Operations may resume |
| `register_asset(policy)` | Authority or active `MANAGE_ASSETS` role | Reject paused config, existing asset, invalid policy, unsupported token semantics | Creates asset and custody token PDA; no tokens | Enabled asset with zero window spend |
| `set_asset_enabled(enabled)` | Authority or active `MANAGE_ASSETS` role | Reject no change or paused config | Updates enabled; no tokens | Asset deposits/new outflows allowed or stopped |
| `update_limits(policy)` | Authority or active `MANAGE_LIMITS` role | Reject paused config or invalid policy; reject a duration change while the current window is live | Updates policy; preserves live window start/spend; after an elapsed window a duration change starts a new zero-spend window at `now`; no tokens | New policy effective immediately without erasing live spend |
| `grant_role(principal, permissions)` | Authority or active `MANAGE_ROLES` role | Reject paused config, default/authority/guardian principal, zero or unknown permissions; non-authority may change only masks that are subsets of its own current mask | Creates or replaces canonical role; no tokens | Active exact permission mask |
| `revoke_role(principal)` | Authority or active `MANAGE_ROLES` role | Reject paused config or inactive/missing role; non-authority cannot revoke a mask containing a permission it lacks | Sets inactive and permissions zero; no tokens | Principal has no role permissions |
| `deposit(amount)` | Any source-token owner signer | Reject disabled asset, zero amount, unsupported token semantics, source mismatch/insufficient funds | Transfers exactly amount source -> asset custody; protocol accounting otherwise unchanged | Custody balance increases |
| `withdraw(amount, recipient_account)` | Authority or active `WITHDRAW` role | Reject paused/disabled, amount above threshold, any limit failure, destination mint/program mismatch, or insufficient custody | Transfers exactly amount custody -> supplied matching-mint account; increments window spend; emits event | Instant outflow complete |
| `request_withdrawal(amount, recipient_account)` | Authority or active `REQUEST_WITHDRAWAL` role | Reject paused/disabled, amount at/below threshold, invalid recipient account, policy maximum failure, ID overflow | Creates immutable request at `next_withdrawal_id`, increments ID; no tokens and no window charge | `Pending` request with fixed destination/time |
| `execute_withdrawal()` | Anyone | Reject paused/disabled, non-`Pending`, early/expired execution, supplied account differs from stored destination, current limit failure, insufficient custody | Transfers exactly recorded amount custody -> recorded token account; increments window spend; sets `Executed`; emits event | Terminal executed request |
| `cancel_withdrawal()` | Authority, guardian, proposer, or active `CANCEL_WITHDRAWAL` role | Reject non-`Pending` | Sets `Cancelled`; no tokens/window charge; emits event | Terminal cancelled request |

`set_vault_authority` changes the stored authority but not the PDA address. The
original namespace authority is retained in `Vault` solely for PDA derivation;
it carries no authorization after rotation.

## 8. Attacker-controlled inputs

Callers can choose transaction account ordering, remaining accounts, source and
destination accounts, token programs, mints, IDs, amounts, timestamps supplied
as instruction arguments, role principals/masks, and policy values. The program
MUST treat each as hostile and bind it to canonical PDA derivations and stored
state. It MUST ignore no unexpected remaining account that could affect token
semantics. No authorization may rely only on a public key passed as data; the
corresponding account must be a transaction signer when required.

## 9. Events and postconditions

Every successful state-changing instruction emits a versioned event containing the affected
state PDA and the minimum immutable identifiers needed to replay transitions.
Outflow events include vault/escrow, asset or mint, amount, fixed destination,
actor, and resulting status/window spend. Events are observability aids, not
authoritative state. An off-chain observer must tolerate duplicate delivery and
reconcile against finalized on-chain accounts and transactions.

The first serialized field after every event discriminator is `version: u16`.
Event names and transition mappings are maintained in `docs/SDK_AND_CLI.md`.

## 10. Upgrade and trust model

Users trust the deployed program code, Solana runtime and Clock sysvar, selected
token program, and the entity controlling upgrade authority. Vault users also
trust their vault authority to configure policy and roles. They do not need to
trust a guardian with custody.

Production deployment requires a reviewed external multisig as program upgrade
authority, a separate external multisig as each high-value vault authority,
documented upgrade delay/review, reproducible build verification, and public
program-data verification. None is proven by this specification.

## 11. Milestone 0 completion map

For every instruction in section 7:

| Required answer | Normative source |
|---|---|
| Who may call it? | Section 7, “Who may call” column; section 6.1 role rules |
| Which accounts can change? | Section 7, “Changes” column; `ACCOUNT_MODEL.md` mutation matrix |
| Which tokens may move and where? | Section 7 token-flow text; global invariants G1–G6 |
| What state is required before execution? | Section 7 rejection conditions; `STATE_MACHINES.md` guards |
| What state exists afterward? | Section 7 result; `STATE_MACHINES.md` target states |
| What can an attacker supply? | Section 8 plus each instruction's argument/account list |
| What must be rejected? | Section 7 rejection conditions, cross-cutting preconditions, and `THREAT_MODEL.md` |

Implementation work is not ready to begin until every added or changed
instruction still has all seven answers and its invariants have negative tests
identified in the threat model.
