use base64::{engine::general_purpose::STANDARD, Engine};
use {
    crate::{
        alerts::{self, Alert},
        database::{Database, EventId, ObservedEvent},
        events::decode_program_data,
        metrics::Metrics,
    },
    anyhow::{anyhow, bail, Context, Result},
    futures_util::{SinkExt, StreamExt},
    serde::Deserialize,
    serde_json::{json, Value},
    std::{
        collections::HashMap,
        sync::{atomic::Ordering, Arc},
        time::Duration,
    },
    tokio::sync::watch,
    tokio_tungstenite::{connect_async, tungstenite::Message},
    tracing::{info, warn},
};

#[derive(Clone, Debug, Default)]
pub struct SyncStatus {
    pub connected: bool,
    pub reconciled: bool,
    pub observed_slot: u64,
    pub rpc_slot: u64,
}

#[derive(Debug, Deserialize)]
struct RpcEnvelope<T> {
    result: Option<T>,
    error: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct SignatureInfo {
    signature: String,
    err: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LogsNotification {
    params: NotificationParams,
}

#[derive(Debug, Deserialize)]
struct NotificationParams {
    result: NotificationResult,
}

#[derive(Debug, Deserialize)]
struct NotificationResult {
    context: SlotContext,
    value: LogValue,
}

#[derive(Debug, Deserialize)]
struct SlotContext {
    slot: u64,
}

#[derive(Debug, Deserialize)]
struct LogValue {
    signature: String,
    err: Option<Value>,
    logs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TransactionResult {
    slot: u64,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    meta: Option<TransactionMeta>,
}

#[derive(Debug, Deserialize)]
struct TransactionMeta {
    err: Option<Value>,
    #[serde(rename = "logMessages")]
    log_messages: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AccountInfoResult {
    value: Option<AccountInfoValue>,
}

#[derive(Debug, Deserialize)]
struct AccountInfoValue {
    data: (String, String),
}

pub struct Observer {
    pub rpc_http_url: String,
    pub rpc_ws_url: String,
    pub program_id: String,
    pub reconnect_delay: Duration,
    pub large_withdrawal_threshold: u64,
    pub database: Database,
    pub metrics: Arc<Metrics>,
    pub status: watch::Sender<SyncStatus>,
    http: reqwest::Client,
}

pub struct ObserverSettings {
    pub rpc_http_url: String,
    pub rpc_ws_url: String,
    pub program_id: String,
    pub reconnect_delay: Duration,
    pub large_withdrawal_threshold: u64,
}

impl Observer {
    pub fn new(
        settings: ObserverSettings,
        database: Database,
        metrics: Arc<Metrics>,
        status: watch::Sender<SyncStatus>,
    ) -> Self {
        Self {
            rpc_http_url: settings.rpc_http_url,
            rpc_ws_url: settings.rpc_ws_url,
            program_id: settings.program_id,
            reconnect_delay: settings.reconnect_delay,
            large_withdrawal_threshold: settings.large_withdrawal_threshold,
            database,
            metrics,
            status,
            http: reqwest::Client::new(),
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut connected_once = false;
        loop {
            if connected_once {
                self.metrics
                    .rpc_reconnects_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            connected_once = true;
            if let Err(error) = self.run_connection().await {
                self.metrics
                    .rpc_errors_total
                    .fetch_add(1, Ordering::Relaxed);
                self.status.send_modify(|state| state.connected = false);
                warn!(%error, "observer connection ended; reconnecting");
                tokio::time::sleep(self.reconnect_delay).await;
            }
        }
    }

    async fn run_connection(&mut self) -> Result<()> {
        let (mut socket, _) = connect_async(&self.rpc_ws_url)
            .await
            .context("connect to Solana WebSocket")?;
        socket
            .send(Message::Text(
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "logsSubscribe",
                    "params": [
                        {"mentions": [self.program_id.clone()]},
                        {"commitment": "finalized"}
                    ]
                })
                .to_string()
                .into(),
            ))
            .await?;

        let acknowledgement = socket
            .next()
            .await
            .context("subscription closed before acknowledgement")??;
        let ack: RpcEnvelope<u64> = serde_json::from_str(acknowledgement.to_text()?)?;
        if let Some(error) = ack.error {
            bail!("logsSubscribe failed: {error}");
        }
        let subscription = ack
            .result
            .context("logsSubscribe returned no subscription ID")?;
        self.status.send_modify(|state| {
            state.connected = true;
            state.reconciled = false;
        });
        info!(subscription, "subscribed to finalized IronVault logs");

        // The subscription is established first. Reconciliation can safely run
        // while live notifications accumulate in the WebSocket receive buffer.
        self.reconcile_to_finalized_slot().await?;

        let mut reconciliation = tokio::time::interval(Duration::from_secs(30));
        reconciliation.tick().await;
        loop {
            tokio::select! {
                message = socket.next() => {
                    let message = message.context("Solana WebSocket closed")??;
                    if !message.is_text() {
                        continue;
                    }
                    let notification: LogsNotification = serde_json::from_str(message.to_text()?)?;
                    let result = notification.params.result;
                    self.status.send_modify(|state| {
                        state.observed_slot = state.observed_slot.max(result.context.slot)
                    });
                    if result.value.err.is_some() {
                        continue;
                    }
                    self.process_logs(
                        &result.value.signature,
                        result.context.slot,
                        None,
                        &result.value.logs,
                    )
                    .await?;
                }
                _ = reconciliation.tick() => {
                    // A logs subscription emits nothing in slots without a
                    // matching transaction. Periodic finalized reconciliation
                    // both repairs missed notifications and advances the
                    // observed slot without assuming socket silence is proof.
                    self.reconcile_to_finalized_slot().await?;
                }
            }
        }
    }

    async fn reconcile_to_finalized_slot(&self) -> Result<()> {
        self.reconcile().await?;
        let reconciled_slot: u64 = self
            .rpc("getSlot", json!([{"commitment": "finalized"}]))
            .await?;
        self.status.send_modify(|state| {
            state.reconciled = true;
            state.rpc_slot = reconciled_slot;
            state.observed_slot = state.observed_slot.max(reconciled_slot);
        });
        Ok(())
    }

    async fn reconcile(&self) -> Result<()> {
        let checkpoint = self.database.checkpoint_signature().await?;
        let mut before: Option<String> = None;
        let mut signatures = Vec::new();
        loop {
            let mut config = json!({"limit": 1000, "commitment": "finalized"});
            if let Some(value) = &before {
                config["before"] = Value::String(value.clone());
            }
            let page: Vec<SignatureInfo> = self
                .rpc("getSignaturesForAddress", json!([self.program_id, config]))
                .await?;
            let page_len = page.len();
            let reached_checkpoint = checkpoint
                .as_ref()
                .is_some_and(|signature| page.iter().any(|entry| &entry.signature == signature));
            before = page.last().map(|entry| entry.signature.clone());
            signatures.extend(page.into_iter().filter(|entry| entry.err.is_none()));
            // Include and replay the checkpoint transaction. A crash may have
            // committed only its first event; the composite event primary key
            // makes persisted siblings no-ops while retaining the rest.
            if reached_checkpoint || page_len < 1000 {
                break;
            }
        }

        signatures.reverse();
        info!(
            transactions = signatures.len(),
            "reconciling finalized program history"
        );
        for info in signatures {
            let transaction: Option<TransactionResult> = self
                .rpc(
                    "getTransaction",
                    json!([info.signature, {
                        "encoding": "json",
                        "commitment": "finalized",
                        "maxSupportedTransactionVersion": 0
                    }]),
                )
                .await?;
            let Some(transaction) = transaction else {
                continue;
            };
            let Some(meta) = transaction.meta else {
                continue;
            };
            if meta.err.is_some() {
                continue;
            }
            if let Some(logs) = meta.log_messages {
                self.process_logs(
                    &info.signature,
                    transaction.slot,
                    transaction.block_time,
                    &logs,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn rpc<T: serde::de::DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        let response = self
            .http
            .post(&self.rpc_http_url)
            .json(&json!({
                "jsonrpc": "2.0", "id": 1, "method": method, "params": params
            }))
            .send()
            .await?
            .error_for_status()?;
        let envelope: RpcEnvelope<T> = response.json().await?;
        if let Some(error) = envelope.error {
            return Err(anyhow!("RPC {method} failed: {error}"));
        }
        envelope.result.context("RPC response omitted result")
    }

    async fn process_logs(
        &self,
        signature: &str,
        slot: u64,
        block_time: Option<i64>,
        logs: &[String],
    ) -> Result<()> {
        let parsed = extract_events(signature, slot, block_time, logs, &self.program_id);
        for item in parsed {
            match item {
                Ok(observed) => {
                    let inserted = self.database.persist(&observed).await?;
                    if inserted {
                        self.record_event(&observed.event);
                        if let Some(alert) =
                            alerts::classify_event(&observed.event, self.large_withdrawal_threshold)
                        {
                            alerts::emit(alert);
                        }
                    }
                }
                Err(error) => {
                    self.metrics
                        .decode_errors_total
                        .fetch_add(1, Ordering::Relaxed);
                    warn!(%signature, %error, "failed to decode IronVault event");
                }
            }
        }
        Ok(())
    }

    fn record_event(&self, event: &crate::events::IronVaultEvent) {
        self.metrics.events_total.fetch_add(1, Ordering::Relaxed);
        use crate::events::IronVaultEvent::*;
        match event {
            EscrowCreated(_) => {
                self.metrics
                    .escrows_created_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            WithdrawalRequested(_) => {
                self.metrics
                    .withdrawals_requested_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            WithdrawalExecuted(_) => {
                self.metrics
                    .withdrawals_executed_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            ProtocolPauseUpdated(event) if event.new_flags != 0 => {
                self.metrics.pauses_total.fetch_add(1, Ordering::Relaxed);
            }
            VaultPauseUpdated(event) if event.paused => {
                self.metrics.pauses_total.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

pub async fn monitor_rpc_slot(
    rpc_http_url: String,
    program_id: String,
    status: watch::Sender<SyncStatus>,
    metrics: Arc<Metrics>,
    max_ready_slot_lag: u64,
) {
    let client = reqwest::Client::new();
    let mut deployment_slot = None;
    loop {
        let result = async {
            let response = client
                .post(&rpc_http_url)
                .json(&json!({
                    "jsonrpc": "2.0", "id": 2, "method": "getSlot",
                    "params": [{"commitment": "finalized"}]
                }))
                .send()
                .await?
                .error_for_status()?;
            let envelope: RpcEnvelope<u64> = response.json().await?;
            if let Some(error) = envelope.error {
                bail!("RPC getSlot failed: {error}");
            }
            envelope.result.context("getSlot omitted result")
        }
        .await;
        match result {
            Ok(slot) => {
                status.send_modify(|state| state.rpc_slot = slot);
                let observed = status.borrow().observed_slot;
                let lag = slot.saturating_sub(observed);
                metrics
                    .observer_slot_lag
                    .store(lag as i64, Ordering::Relaxed);
                if lag > max_ready_slot_lag {
                    alerts::emit(Alert::ObserverFallingBehind { lag });
                }
                match fetch_deployment_slot(&client, &rpc_http_url, &program_id).await {
                    Ok(current) => {
                        if deployment_slot.is_some_and(|previous| previous != current) {
                            alerts::emit(Alert::ProgramUpgradeObserved);
                        }
                        deployment_slot = Some(current);
                    }
                    Err(error) => {
                        metrics.rpc_errors_total.fetch_add(1, Ordering::Relaxed);
                        warn!(%error, "failed to inspect program deployment slot");
                    }
                }
            }
            Err(error) => {
                metrics.rpc_errors_total.fetch_add(1, Ordering::Relaxed);
                warn!(%error, "failed to refresh finalized RPC slot");
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn fetch_deployment_slot(
    client: &reqwest::Client,
    rpc_http_url: &str,
    program_id: &str,
) -> Result<u64> {
    let program = fetch_account_data(client, rpc_http_url, program_id).await?;
    if program.len() < 36 || u32::from_le_bytes(program[..4].try_into()?) != 2 {
        bail!("program account is not an upgradeable-loader Program state");
    }
    let program_data_address = bs58::encode(&program[4..36]).into_string();
    let program_data = fetch_account_data(client, rpc_http_url, &program_data_address).await?;
    if program_data.len() < 12 || u32::from_le_bytes(program_data[..4].try_into()?) != 3 {
        bail!("program-data account has an invalid upgradeable-loader state");
    }
    Ok(u64::from_le_bytes(program_data[4..12].try_into()?))
}

async fn fetch_account_data(
    client: &reqwest::Client,
    rpc_http_url: &str,
    address: &str,
) -> Result<Vec<u8>> {
    let response = client
        .post(rpc_http_url)
        .json(&json!({
            "jsonrpc": "2.0", "id": 3, "method": "getAccountInfo",
            "params": [address, {"encoding": "base64", "commitment": "finalized"}]
        }))
        .send()
        .await?
        .error_for_status()?;
    let envelope: RpcEnvelope<AccountInfoResult> = response.json().await?;
    if let Some(error) = envelope.error {
        bail!("RPC getAccountInfo failed: {error}");
    }
    let account = envelope
        .result
        .and_then(|result| result.value)
        .context("account does not exist")?;
    if account.data.1 != "base64" {
        bail!("RPC returned an unexpected account encoding");
    }
    STANDARD
        .decode(account.data.0)
        .context("invalid account base64")
}

pub fn extract_events(
    signature: &str,
    slot: u64,
    block_time: Option<i64>,
    logs: &[String],
    program_id: &str,
) -> Vec<Result<ObservedEvent>> {
    let mut stack: Vec<String> = Vec::new();
    let mut instruction_index: i32 = -1;
    let mut event_indexes: HashMap<u32, u32> = HashMap::new();
    let mut output = Vec::new();

    for log in logs {
        if let Some((program, depth)) = parse_invoke(log) {
            if depth == 1 {
                instruction_index += 1;
                stack.clear();
            }
            stack.push(program.to_owned());
            continue;
        }
        if log.starts_with("Program ") && (log.ends_with(" success") || log.contains(" failed:")) {
            stack.pop();
            continue;
        }
        if stack.last().is_some_and(|current| current == program_id)
            && log.starts_with("Program data: ")
        {
            let index =
                u32::try_from(instruction_index.max(0)).expect("nonnegative instruction index");
            let event_index = event_indexes.entry(index).or_default();
            let id = EventId {
                transaction_signature: signature.to_owned(),
                instruction_index: index,
                event_index: *event_index,
            };
            *event_index += 1;
            output.push(decode_program_data(log).and_then(|event| {
                event
                    .map(|event| ObservedEvent {
                        id,
                        slot,
                        block_time,
                        event,
                    })
                    .context("program-data line did not contain an event")
            }));
        }
    }
    output
}

fn parse_invoke(log: &str) -> Option<(&str, u32)> {
    let rest = log.strip_prefix("Program ")?;
    let (program, suffix) = rest.split_once(" invoke [")?;
    let depth = suffix.strip_suffix(']')?.parse().ok()?;
    Some((program, depth))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::events::{discriminator, Pubkey, VaultPauseUpdated},
        base64::{engine::general_purpose::STANDARD, Engine},
    };

    #[test]
    fn assigns_stable_instruction_and_event_indexes_across_cpi() {
        let program = "Iron1111111111111111111111111111111111111";
        let event = VaultPauseUpdated {
            version: 1,
            vault: Pubkey([1; 32]),
            caller: Pubkey([2; 32]),
            paused: true,
        };
        let mut bytes = discriminator("VaultPauseUpdated").to_vec();
        bytes.extend(borsh::to_vec(&event).unwrap());
        let data = STANDARD.encode(bytes);
        let logs = vec![
            "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_owned(),
            "Program ComputeBudget111111111111111111111111111111 success".to_owned(),
            "Program Multisig111111111111111111111111111111111 invoke [1]".to_owned(),
            format!("Program {program} invoke [2]"),
            format!("Program data: {data}"),
            format!("Program data: {data}"),
            format!("Program {program} success"),
            "Program Multisig111111111111111111111111111111111 success".to_owned(),
        ];
        let events = extract_events("sig", 42, None, &logs, program)
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id.instruction_index, 1);
        assert_eq!(events[0].id.event_index, 0);
        assert_eq!(events[1].id.event_index, 1);
    }

    #[test]
    fn ignores_events_emitted_by_other_programs() {
        let logs = vec![
            "Program Other invoke [1]".to_owned(),
            "Program data: invalid".to_owned(),
            "Program Other success".to_owned(),
        ];
        assert!(extract_events("sig", 1, None, &logs, "Iron").is_empty());
    }
}
