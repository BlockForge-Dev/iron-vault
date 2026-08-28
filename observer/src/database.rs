use {
    crate::events::IronVaultEvent,
    anyhow::{Context, Result},
    serde::Serialize,
    sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Transaction},
    std::path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EventId {
    pub transaction_signature: String,
    pub instruction_index: u32,
    pub event_index: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObservedEvent {
    pub id: EventId,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub event: IronVaultEvent,
}

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await
            .context("connect to PostgreSQL")?;
        sqlx::migrate::Migrator::new(Path::new("./migrations"))
            .await
            .context("load observer database migrations")?
            .run(&pool)
            .await
            .context("run observer database migrations")?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn checkpoint_signature(&self) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT transaction_signature FROM sync_checkpoint WHERE singleton = TRUE",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|value| value.0))
    }

    /// Atomically stores the immutable event, applies its read model, and moves
    /// the monitoring checkpoint. A replay of the same event ID is a no-op.
    pub async fn persist(&self, observed: &ObservedEvent) -> Result<bool> {
        let slot = i64::try_from(observed.slot).context("slot exceeds PostgreSQL BIGINT")?;
        let instruction = i32::try_from(observed.id.instruction_index)?;
        let event_index = i32::try_from(observed.id.event_index)?;
        let payload = observed.event.payload();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO protocol_events
             (transaction_signature, instruction_index, event_index, slot, block_time,
              event_name, schema_version, payload)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT DO NOTHING",
        )
        .bind(&observed.id.transaction_signature)
        .bind(instruction)
        .bind(event_index)
        .bind(slot)
        .bind(observed.block_time)
        .bind(observed.event.name())
        .bind(i16::try_from(observed.event.version())?)
        .bind(payload)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;

        if inserted {
            apply_projection(&mut tx, observed, slot, instruction, event_index).await?;
        }
        update_checkpoint(&mut tx, observed, slot, instruction, event_index).await?;
        tx.commit().await?;
        Ok(inserted)
    }
}

async fn apply_projection(
    tx: &mut Transaction<'_, Postgres>,
    observed: &ObservedEvent,
    slot: i64,
    instruction: i32,
    event_index: i32,
) -> Result<()> {
    let signature = &observed.id.transaction_signature;
    match &observed.event {
        IronVaultEvent::EscrowCreated(event) => {
            sqlx::query(
                "INSERT INTO escrows
                 (escrow, escrow_token, maker, recipient, mint, amount, escrow_id,
                  created_at, expires_at, status, last_slot, last_instruction_index,
                  last_event_index, last_signature)
                 VALUES ($1, $2, $3, $4, $5, $6::numeric, $7::numeric, $8, $9,
                         'funded', $10, $11, $12, $13)
                 ON CONFLICT (escrow) DO NOTHING",
            )
            .bind(event.escrow.to_string())
            .bind(event.escrow_token.to_string())
            .bind(event.maker.to_string())
            .bind(event.recipient.to_string())
            .bind(event.mint.to_string())
            .bind(event.amount.to_string())
            .bind(event.escrow_id.to_string())
            .bind(event.created_at)
            .bind(event.expires_at)
            .bind(slot)
            .bind(instruction)
            .bind(event_index)
            .bind(signature)
            .execute(&mut **tx)
            .await?;
        }
        IronVaultEvent::EscrowReleased(event) => {
            update_escrow_status(
                tx,
                &event.escrow.to_string(),
                "released",
                observed,
                slot,
                instruction,
                event_index,
            )
            .await?;
        }
        IronVaultEvent::EscrowRefunded(event) => {
            update_escrow_status(
                tx,
                &event.escrow.to_string(),
                "refunded",
                observed,
                slot,
                instruction,
                event_index,
            )
            .await?;
        }
        IronVaultEvent::VaultCreated(event) => {
            sqlx::query(
                "INSERT INTO vaults
                 (vault, namespace_authority, authority, guardian, vault_id, paused,
                  last_slot, last_instruction_index, last_event_index, last_signature)
                 VALUES ($1, $2, $3, $4, $5::numeric, FALSE, $6, $7, $8, $9)
                 ON CONFLICT (vault) DO NOTHING",
            )
            .bind(event.vault.to_string())
            .bind(event.namespace_authority.to_string())
            .bind(event.authority.to_string())
            .bind(event.guardian.to_string())
            .bind(event.vault_id.to_string())
            .bind(slot)
            .bind(instruction)
            .bind(event_index)
            .bind(signature)
            .execute(&mut **tx)
            .await?;
        }
        IronVaultEvent::VaultAuthorityUpdated(event) => {
            sqlx::query(
                "UPDATE vaults SET authority = $1, last_slot = $2,
                 last_instruction_index = $3, last_event_index = $4, last_signature = $5
                 WHERE vault = $6 AND
                 (last_slot, last_instruction_index, last_event_index) < ($2, $3, $4)",
            )
            .bind(event.new_authority.to_string())
            .bind(slot)
            .bind(instruction)
            .bind(event_index)
            .bind(signature)
            .bind(event.vault.to_string())
            .execute(&mut **tx)
            .await?;
        }
        IronVaultEvent::VaultPauseUpdated(event) => {
            sqlx::query(
                "UPDATE vaults SET paused = $1, last_slot = $2,
                 last_instruction_index = $3, last_event_index = $4, last_signature = $5
                 WHERE vault = $6 AND
                 (last_slot, last_instruction_index, last_event_index) < ($2, $3, $4)",
            )
            .bind(event.paused)
            .bind(slot)
            .bind(instruction)
            .bind(event_index)
            .bind(signature)
            .bind(event.vault.to_string())
            .execute(&mut **tx)
            .await?;
        }
        IronVaultEvent::WithdrawalRequested(event) => {
            sqlx::query(
                "INSERT INTO withdrawal_requests
                 (withdrawal_request, vault, vault_asset, proposer, recipient_owner,
                  recipient_token_account, mint, withdrawal_id, amount, created_at,
                  execute_after, expires_at, status, last_slot, last_instruction_index,
                  last_event_index, last_signature)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8::numeric, $9::numeric,
                         $10, $11, $12, 'pending', $13, $14, $15, $16)
                 ON CONFLICT (withdrawal_request) DO NOTHING",
            )
            .bind(event.withdrawal_request.to_string())
            .bind(event.vault.to_string())
            .bind(event.vault_asset.to_string())
            .bind(event.proposer.to_string())
            .bind(event.recipient_owner.to_string())
            .bind(event.recipient_token_account.to_string())
            .bind(event.mint.to_string())
            .bind(event.withdrawal_id.to_string())
            .bind(event.amount.to_string())
            .bind(event.created_at)
            .bind(event.execute_after)
            .bind(event.expires_at)
            .bind(slot)
            .bind(instruction)
            .bind(event_index)
            .bind(signature)
            .execute(&mut **tx)
            .await?;
        }
        IronVaultEvent::WithdrawalExecuted(event) => {
            update_withdrawal_status(
                tx,
                &event.withdrawal_request.to_string(),
                "executed",
                observed,
                slot,
                instruction,
                event_index,
            )
            .await?;
        }
        IronVaultEvent::WithdrawalCancelled(event) => {
            update_withdrawal_status(
                tx,
                &event.withdrawal_request.to_string(),
                "cancelled",
                observed,
                slot,
                instruction,
                event_index,
            )
            .await?;
        }
        _ => {}
    }
    Ok(())
}

async fn update_escrow_status(
    tx: &mut Transaction<'_, Postgres>,
    escrow: &str,
    status: &str,
    observed: &ObservedEvent,
    slot: i64,
    instruction: i32,
    event_index: i32,
) -> Result<()> {
    sqlx::query(
        "UPDATE escrows SET status = $1, last_slot = $2, last_instruction_index = $3,
         last_event_index = $4, last_signature = $5 WHERE escrow = $6 AND
         (last_slot, last_instruction_index, last_event_index) < ($2, $3, $4)",
    )
    .bind(status)
    .bind(slot)
    .bind(instruction)
    .bind(event_index)
    .bind(&observed.id.transaction_signature)
    .bind(escrow)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn update_withdrawal_status(
    tx: &mut Transaction<'_, Postgres>,
    request: &str,
    status: &str,
    observed: &ObservedEvent,
    slot: i64,
    instruction: i32,
    event_index: i32,
) -> Result<()> {
    sqlx::query(
        "UPDATE withdrawal_requests SET status = $1, last_slot = $2,
         last_instruction_index = $3, last_event_index = $4, last_signature = $5
         WHERE withdrawal_request = $6 AND
         (last_slot, last_instruction_index, last_event_index) < ($2, $3, $4)",
    )
    .bind(status)
    .bind(slot)
    .bind(instruction)
    .bind(event_index)
    .bind(&observed.id.transaction_signature)
    .bind(request)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn update_checkpoint(
    tx: &mut Transaction<'_, Postgres>,
    observed: &ObservedEvent,
    slot: i64,
    instruction: i32,
    event_index: i32,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO sync_checkpoint
         (singleton, slot, transaction_signature, instruction_index, event_index)
         VALUES (TRUE, $1, $2, $3, $4)
         ON CONFLICT (singleton) DO UPDATE SET
             slot = EXCLUDED.slot,
             transaction_signature = EXCLUDED.transaction_signature,
             instruction_index = EXCLUDED.instruction_index,
             event_index = EXCLUDED.event_index,
             updated_at = now()
         WHERE (sync_checkpoint.slot, sync_checkpoint.instruction_index,
                sync_checkpoint.event_index) <
               (EXCLUDED.slot, EXCLUDED.instruction_index, EXCLUDED.event_index)",
    )
    .bind(slot)
    .bind(&observed.id.transaction_signature)
    .bind(instruction)
    .bind(event_index)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
