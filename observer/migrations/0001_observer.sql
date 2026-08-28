CREATE TABLE protocol_events (
    transaction_signature TEXT NOT NULL,
    instruction_index INTEGER NOT NULL CHECK (instruction_index >= 0),
    event_index INTEGER NOT NULL CHECK (event_index >= 0),
    slot BIGINT NOT NULL CHECK (slot >= 0),
    block_time BIGINT,
    event_name TEXT NOT NULL,
    schema_version SMALLINT NOT NULL CHECK (schema_version > 0),
    payload JSONB NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (transaction_signature, instruction_index, event_index)
);

CREATE INDEX protocol_events_slot_idx
    ON protocol_events (slot, instruction_index, event_index);
CREATE INDEX protocol_events_name_idx ON protocol_events (event_name);

CREATE TABLE escrows (
    escrow TEXT PRIMARY KEY,
    escrow_token TEXT NOT NULL,
    maker TEXT NOT NULL,
    recipient TEXT NOT NULL,
    mint TEXT NOT NULL,
    amount NUMERIC(20, 0) NOT NULL CHECK (amount > 0),
    escrow_id NUMERIC(20, 0) NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('funded', 'released', 'refunded')),
    last_slot BIGINT NOT NULL,
    last_instruction_index INTEGER NOT NULL,
    last_event_index INTEGER NOT NULL,
    last_signature TEXT NOT NULL
);

CREATE TABLE vaults (
    vault TEXT PRIMARY KEY,
    namespace_authority TEXT NOT NULL,
    authority TEXT NOT NULL,
    guardian TEXT NOT NULL,
    vault_id NUMERIC(20, 0) NOT NULL,
    paused BOOLEAN NOT NULL DEFAULT FALSE,
    last_slot BIGINT NOT NULL,
    last_instruction_index INTEGER NOT NULL,
    last_event_index INTEGER NOT NULL,
    last_signature TEXT NOT NULL
);

CREATE TABLE withdrawal_requests (
    withdrawal_request TEXT PRIMARY KEY,
    vault TEXT NOT NULL,
    vault_asset TEXT NOT NULL,
    proposer TEXT NOT NULL,
    recipient_owner TEXT NOT NULL,
    recipient_token_account TEXT NOT NULL,
    mint TEXT NOT NULL,
    withdrawal_id NUMERIC(20, 0) NOT NULL,
    amount NUMERIC(20, 0) NOT NULL CHECK (amount > 0),
    created_at BIGINT NOT NULL,
    execute_after BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'executed', 'cancelled')),
    last_slot BIGINT NOT NULL,
    last_instruction_index INTEGER NOT NULL,
    last_event_index INTEGER NOT NULL,
    last_signature TEXT NOT NULL
);

CREATE TABLE sync_checkpoint (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    slot BIGINT NOT NULL CHECK (slot >= 0),
    transaction_signature TEXT NOT NULL,
    instruction_index INTEGER NOT NULL CHECK (instruction_index >= 0),
    event_index INTEGER NOT NULL CHECK (event_index >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
