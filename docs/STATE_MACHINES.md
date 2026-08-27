# State Machines

State transitions are closed sets: a transition not listed here is forbidden.
Time-derived labels such as “executable” are views over a stored state, not
additional serialized variants.

## 1. Escrow

```text
create + atomic funding
          |
          v
       Funded
       /    \
 maker release   permissionless refund
 now < expiry    now >= expiry
     /              \
    v                v
 Released         Refunded
```

| From | Instruction | Guard | Token effect | To |
|---|---|---|---|---|
| absent | `create_escrow` | valid signer, terms, source, token semantics, no creation pause | exact amount source -> custody | Funded |
| Funded | `release_escrow` | maker signer, before expiry, no release pause, fixed recipient | exact amount custody -> recipient | Released |
| Funded | `refund_escrow` | at/after expiry, fixed maker destination | exact amount custody -> maker | Refunded |

Released and Refunded are terminal. The implementation checks status before CPI
and writes the terminal status in the same transaction. A failed CPI rolls the
whole transaction back. Extra tokens sent directly to custody are not part of
`amount`; v1 provides no recovery path for unsolicited excess and clients MUST
warn against direct transfers.

## 2. Vault pause state

```text
             authority or guardian
     Active ------------------------> Paused
       ^                                |
       |          authority only        |
       +--------------------------------+
```

Active permits operations subject to protocol pause, asset enablement, roles,
and policy. Paused permits deposits, cancellation, and configuration but blocks
new or executing outflows. This allows remediation before the authority
unpauses. Guardian rotation does not unpause a vault. A protocol-level
`VAULT_CONFIG` pause independently blocks configuration as specified in the
protocol pause matrix.

## 3. Asset lifecycle

```text
 absent --register--> Enabled <----set enabled----> Disabled
```

Registration creates both policy state and canonical custody. Disabled is not a
terminal state and does not move funds. Disabling blocks deposits, instant
withdrawals, requests, and execution; cancellation remains possible.

## 4. Withdrawal request

```text
request (amount > threshold)
          |
          v
        Pending
       /   |    \
      /    |     \
 early     |      now > expires_at
(no edge)  |        (no edge; cancellable)
           |
 execute_after <= now <= expires_at
           |
           +---- execute ----> Executed
           |
           +---- cancel -----> Cancelled
```

`Executable` and `Expired` are computed views of `Pending`:

- `PendingEarly`: `now < execute_after`
- `Executable`: `execute_after <= now <= expires_at`
- `Expired`: `now > expires_at`

Expired requests do not execute and remain cancellable. Executed and Cancelled
are terminal. Execution is permissionless because the exact recipient token
account and amount are immutable; the cranker has no destination authority.

## 5. Rolling-window accounting

For each asset, the first successful outflow in an elapsed window starts a new
window at the current Clock time and sets spend to that outflow amount. Within a
live window, each successful outflow adds its amount. Requests and cancellations
do not spend capacity.

```text
attempt outflow
      |
      v
window elapsed? --yes--> start=now, spent=0
      | no
      v
checked(spent + amount) <= limit?
      | yes                         | no
      v                             v
token CPI succeeds             reject; no mutation
      |
      v
spent += amount, emit event
```

Solana transaction atomicity serializes competing writes to one `VaultAsset`.
Two transactions cannot both consume the same remaining window capacity from
the same pre-state.

## 6. Role lifecycle

```text
absent/revoked --grant exact mask--> Active --revoke--> Revoked
                      ^                 |
                      +--replace mask---+
```

Authorization reads current account data on each instruction. There is no cached
permission in a withdrawal request: permission is needed to create it, but
execution is later permissionless and constrained by the immutable request.
Revoking a proposer therefore prevents new requests but does not silently cancel
an existing one; guardian or another cancellation authority must cancel it.

## 7. Configuration transitions

Authority and guardian rotation are immediate and single-step in v1. This avoids
half-configured pending state but creates key-entry risk; production vaults
SHOULD use a multisig authority. A future two-step authority acceptance flow is
an explicitly deferred hardening item.

Limit updates never mutate request data or terminal records. A live window's
duration cannot change, preventing a policy manager from resetting or shortening
it to recover capacity. After a window has elapsed, a duration change starts a
fresh window at current Clock time with zero spend. Updating other limit fields
preserves current window start and spend; lowering the limit below current spend
simply blocks further outflow until the window elapses.
