# Read-only observer

The IronVault observer is an optional operational read model. It improves audit
history, analytics, monitoring, API convenience, and alert delivery. It is
never authoritative: clients making security decisions MUST read finalized
Solana state. Deleting and rebuilding the observer database cannot change funds
or program state.

## Data flow and trust boundary

The service establishes a `finalized` `logsSubscribe` WebSocket subscription
before reconciling finalized signature history over HTTP RPC. This ordering
closes the subscribe/backfill race. On reconnect it deliberately reprocesses the
checkpoint transaction, because a crash may occur between two events in the
same transaction.

Only `Program data:` emitted while IronVault is the active invocation is
decoded. A CPI-aware invocation stack rejects data logged by other programs.
Each Anchor discriminator and exact Borsh payload is decoded locally. Unknown,
malformed, or trailing data fails closed and increments the decode-error metric.

Every event is identified by:

```text
(transaction_signature, instruction_index, event_index)
```

That tuple is the PostgreSQL primary key. Event insertion, read-model update,
and checkpoint advancement share one database transaction. Reconciliation is
therefore at-least-once at the RPC boundary and exactly-once per event identity
inside the database.

## Storage

`protocol_events` is the append-only decoded event history. `escrows`, `vaults`,
and `withdrawal_requests` are rebuildable projections. `sync_checkpoint` records
the newest committed event position. Projection updates compare
`(slot, instruction_index, event_index)`, so replaying older history cannot roll
state backward.

The database does not attempt to infer transfers from balances or failed
transactions. It records only successfully decoded events from finalized,
successful transactions. PostgreSQL `NUMERIC(20,0)` preserves the full Solana
`u64` range.

## HTTP and metrics

- `GET /healthz` proves the process and HTTP runtime are alive.
- `GET /readyz` requires PostgreSQL, a connected and reconciled WebSocket, and
  slot lag no greater than `IRONVAULT_MAX_READY_SLOT_LAG`.
- `GET /metrics` exposes Prometheus text format.

Metrics are:

- `ironvault_events_total`
- `ironvault_rpc_errors_total`
- `ironvault_rpc_reconnects_total`
- `ironvault_observer_slot_lag`
- `ironvault_decode_errors_total`
- `ironvault_escrows_created_total`
- `ironvault_withdrawals_requested_total`
- `ironvault_withdrawals_executed_total`
- `ironvault_pauses_total`

Structured warning records cover protocol/vault pauses, large withdrawal
requests, vault-authority changes, program deployment-slot changes, and observer
lag. Alert routing is intentionally backend-dependent: production deployments
ship these records to their paging/logging system. Protocol and vault guardians
are immutable in v1, so no successful guardian-change event can occur. If a
future instruction makes either guardian mutable, it MUST add a versioned event
and connect the existing `GuardianChanged` alert before release.

## Local operation

```bash
make local-up
curl --fail http://127.0.0.1:8080/healthz
curl --fail http://127.0.0.1:8080/readyz
curl --fail http://127.0.0.1:8080/metrics
make e2e
make local-down
```

Compose runs Agave 3.1.10, PostgreSQL 17.6, the observer, and Prometheus 3.6.0.
The local password is explicitly development-only. Copy `.env.example` for a
non-Compose observer process and replace all credentials.

The pinned `anzaxyz/agave:v3.1.10` image is local-development infrastructure.
Its publisher has sunset that Docker Hub repository, so it MUST NOT be adopted
as a maintained production base image. Replace and re-review the validator image
when the pinned Solana toolchain changes.

## Non-guarantees

- The observer cannot make a transaction valid, final, or reversible.
- WebSocket delivery is not assumed reliable; RPC reconciliation is mandatory.
- `/healthz` does not imply caught-up data; use `/readyz`.
- Metrics counters restart at zero with the process; durable history remains in
  PostgreSQL.
- Upgrade detection compares the upgradeable loader's finalized ProgramData
  deployment slot every five seconds. It is monitoring, not proof of approved
  governance or bytecode equivalence.
