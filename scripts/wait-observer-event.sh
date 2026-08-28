#!/usr/bin/env bash
set -euo pipefail

signature="$(tr -d '\r\n' < target/e2e-signature.txt)"
for _ in $(seq 1 60); do
  count="$(docker compose exec -T postgres psql -U iron_vault -d iron_vault -Atc \
    "SELECT count(*) FROM protocol_events WHERE event_name = 'ProtocolInitialized' AND transaction_signature = '$signature'")"
  if [[ "$count" -eq 1 ]]; then
    curl --fail --silent http://127.0.0.1:8080/readyz
    printf '\nObserver persisted ProtocolInitialized exactly once or replayed it idempotently.\n'
    exit 0
  fi
  sleep 1
done

echo "Observer did not persist ProtocolInitialized for $signature within 60 seconds" >&2
exit 1
