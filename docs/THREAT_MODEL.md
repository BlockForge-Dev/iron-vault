# Threat Model

## 1. Security objective

IronVault aims to ensure that tokens in program custody move only through the
documented state machines to destinations authorized by immutable escrow terms
or vault policy. It aims to contain a compromised guardian to denial of service,
not loss of custody.

This is a design analysis. It is not an audit, implementation evidence, or a
claim of mainnet readiness.

## 2. Protected assets

- Tokens in escrow and vault custody accounts
- Immutable escrow and withdrawal intent
- Vault role, pause, policy, and rolling-window state
- Protocol administration and vault authority identities
- Accurate, replayable events and terminal-state history
- Separation between operational and program upgrade authority

## 3. Trust boundaries and assumptions

| Component / actor | Trusted for | Not trusted for |
|---|---|---|
| Solana runtime | signer enforcement, ownership, atomic transactions, account locks | Application policy |
| Clock sysvar | on-chain time used by guards | Precise wall-clock scheduling |
| SPL Token and Token-2022 programs | documented base transfer/account semantics | Semantics of Token-2022 extensions rejected by policy |
| Program upgrade authority | Can replace all program logic | Custody safety after compromise |
| Vault authority | Roles, policy, destinations it directly authorizes | Protocol-global administration |
| Protocol admin | Protocol config and global pause | Withdrawing user funds |
| Guardians | Emergency pause/cancellation | Any token transfer or unpause |
| Operators/crankers/users | Nothing beyond valid signatures for their own keys | Supplied accounts, IDs, values, ordering, timing claims |
| Off-chain observer | Availability/analytics | Consensus or authorization |

The system cannot protect users from a malicious program upgrade. Upgrade
governance, reproducible builds, deployment verification, and multisig policy
are therefore critical external controls.

## 4. Adversary capabilities

An attacker may:

- submit arbitrary transactions and reorder, retry, or race them;
- choose every non-PDA account and every instruction argument;
- own valid token accounts and create mints with hostile Token-2022 extensions;
- compromise an operator or guardian key;
- directly transfer unsolicited tokens into a custody address;
- monitor pending requests and execute permissionless operations at the earliest
  valid slot;
- cause RPC/event delivery duplication, delay, rollback before finality, or
  omission;
- exploit integer boundaries, timestamp boundaries, duplicate mutable accounts,
  confused-deputy CPIs, PDA substitution, or cross-vault account mixing.

The adversary is assumed unable to forge signatures, find PDA collisions, break
Solana runtime isolation, or alter finalized ledger state.

## 5. Threats and required mitigations

| ID | Threat | Required mitigation | Residual / non-guarantee |
|---|---|---|---|
| T1 | Fake state or custody PDA | Typed owner checks, canonical seeds, stored parent/mint revalidation | Framework/runtime bugs remain external |
| T2 | Destination substitution on refund/release | Check destination token owner and mint against immutable maker/recipient and mint | Beneficiary controls its own token account after receipt |
| T3 | Request changed after timelock | Store exact destination, owner, mint, amount, and times; only status mutable | Authority can cancel and create a new request with a new delay |
| T4 | Replay/double withdrawal | Terminal status tombstone, status check before CPI, atomic CPI/state write | Client retries may fail after first success and must reconcile |
| T5 | Timelock bypass | Instant path requires amount <= threshold; request path records Clock-derived execution time | Validator clock is approximate, not wall-clock exact |
| T6 | Rolling-limit race/reset | One mutable asset account is locked; checked arithmetic updates spend atomically; live-window duration changes are rejected and other updates retain spend | Limits are token units, not fiat value |
| T7 | Guardian drains funds | Guardian has pause/cancel only; destination cannot be supplied to those instructions | Compromised guardian can deny service |
| T8 | Protocol admin drains funds | No admin custody signer path or rescue transfer instruction | Malicious upgrade authority can add one in a future binary |
| T9 | Malicious operator | Least-privilege bitmask, per-call live role check, limits/timelocks, fixed request | Authorized instant withdrawals may choose any matching-mint destination |
| T10 | Hostile Token-2022 behavior | Parse mint TLV data and accept only an empty extension list; reject Permanent Delegate, Transfer Hook, Transfer Fee, Non-transferable, and unknown extensions | Accepted mint authorities may still inflate supply or freeze accounts where base semantics support it |
| T11 | Wrong token program / mint | Store and constrain both; token account/mint owner and relationship checks | Bugs in supported token program are external |
| T12 | Direct custody transfer | State tracks intended amount/policy, never equates raw balance with entitlement | Unsolicited excess can be stranded in v1 |
| T13 | Account close/rent theft | Fixed close recipients; retain state tombstones | Rent remains locked in non-closed state accounts |
| T14 | Pause abused to trap escrow | Expired refunds bypass pause | Pre-expiry release may be delayed until expiry/refund path |
| T15 | Policy loosened around pending request | Request timelock immutable; current execution limits applied | Authority can loosen monetary limits; authority is trusted for policy |
| T16 | Role self-escalation | Exact masks, known bits only, no authority-only permissions; non-authority managers may change only masks contained in their own live mask | A manager holding broad permissions can delegate those same permissions |
| T17 | Stale/off-chain event view | Versioned events, idempotent observer, finalized reconciliation with state | Observer API is not authoritative and may lag |
| T18 | Upgrade-key compromise | Separate multisig, review/delay, reproducible build and program-data verification | On-chain v1 cannot enforce the external governance procedure |
| T19 | Arithmetic/time overflow | Checked add/subtract, bounded positive durations and amounts | Extreme valid timestamps remain bounded by `i64` |
| T20 | Remaining-account extension trick | Reject unexpected accounts and unsupported extensions; explicit CPI account list | Future extension support expands audit surface |

## 6. Key-compromise outcomes

| Compromised key | Maximum intended impact |
|---|---|
| Escrow maker | May release its own funded escrows before expiry to the already fixed recipient |
| Operator with `WITHDRAW` | May withdraw within current instant and rolling limits to a matching-mint account |
| Operator with `REQUEST_WITHDRAWAL` | May create visible delayed requests; cannot execute early or mutate them |
| Vault guardian | May pause and cancel pending requests; cannot transfer/unpause |
| Vault authority | May change roles/policy and authorize vault outflows; vault custody is compromised by design |
| Protocol guardian | May add global pause flags only |
| Protocol admin | May change protocol configuration/pause but has no direct custody path |
| Program upgrade authority | Total protocol compromise; can deploy arbitrary replacement logic |

## 7. Failure modes

All validation failures return explicit custom errors and cause no state or token
change. Important fail-closed cases include paused/disabled state, unsupported
token semantics, invalid PDA/parent relation, wrong signer/role, stale status,
boundary-time failure, insufficient balance/capacity, overflow, unknown flags or
permissions, and failed token CPI.

Loss of RPC or observer service does not affect on-chain safety. Solana outage or
congestion may delay release/refund/withdrawal. A timelock is a minimum delay,
not an execution guarantee. A beneficiary's frozen token account can make a
fixed-destination transfer fail; v1 intentionally provides no redirection.

## 8. Security requirements for implementation tests

Every invariant and transition requires positive and negative tests. At minimum:

- substitution of each parent, mint, token program, custody, and destination;
- missing/wrong signer and every role bit independently;
- release/refund at `expiry - 1`, `expiry`, and `expiry + 1`;
- execute at `execute_after - 1`, both inclusive boundaries, and after expiry;
- repeat and cross-terminal escrow/request calls;
- threshold/maximum/window values at minus one, equality, plus one, zero, and
  integer overflow boundaries;
- concurrent attempts consuming the final window capacity;
- guardian attempts to withdraw, unpause, or clear global pause flags;
- paused and disabled behavior for every instruction;
- direct excess transfer into custody;
- hostile/mismatched token accounts and Token-2022 mints/extensions;
- failed CPI proving rollback of status and window accounting;
- authority and guardian rotation invalidating old keys immediately;
- duplicate event ingestion and finalized-state reconciliation.

Fuzz/property tests SHOULD encode the invariants independently of handler code.
Deployment readiness additionally requires deterministic builds, deployment
hash verification, secret scanning, upgrade-authority verification, and an
independent security review.

## 9. Known limitations and deferred decisions

- Only extension-free Token-2022 mints are supported; all mint extensions are
  denied until their semantics and CPI account requirements receive explicit
  protocol support.
- Vault authority compromise is not recoverable by the protocol guardian.
- Authority rotation is one-step rather than accept/confirm.
- There is no recovery for tokens sent directly to custody in excess of tracked
  escrow obligations, nor for unsupported tokens sent to an address.
- Fixed beneficiary accounts can become frozen or otherwise unusable.
- No oracle means policies cannot express fiat-denominated risk limits.
- Tombstones consume rent indefinitely and require a future migration/archival
  design before closure can be safe.
- Program upgrade separation and multisig thresholds are operationally enforced,
  not program-enforced.
