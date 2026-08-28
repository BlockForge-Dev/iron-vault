use {
    anyhow::{Context, Result},
    clap::Parser,
    iron_vault_observer::{
        config::Config,
        database::Database,
        health::{self, HealthState},
        metrics::Metrics,
        rpc::{monitor_rpc_slot, Observer, ObserverSettings, SyncStatus},
    },
    std::{sync::Arc, time::Duration},
    tokio::{net::TcpListener, sync::watch},
    tracing::info,
    tracing_subscriber::EnvFilter,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let database = Database::connect(&config.database_url).await?;
    let metrics = Arc::new(Metrics::default());
    let (status_tx, status_rx) = watch::channel(SyncStatus::default());
    let app = health::router(HealthState {
        database: database.pool().clone(),
        metrics: Arc::clone(&metrics),
        sync_status: status_rx,
        max_ready_slot_lag: config.max_ready_slot_lag,
    });
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("bind observer HTTP server to {}", config.listen_addr))?;
    info!(address = %config.listen_addr, program_id = %config.program_id, "IronVault observer starting");

    let observer = Observer::new(
        ObserverSettings {
            rpc_http_url: config.rpc_http_url.clone(),
            rpc_ws_url: config.rpc_ws_url,
            program_id: config.program_id.clone(),
            reconnect_delay: Duration::from_millis(config.reconnect_delay_ms),
            large_withdrawal_threshold: config.large_withdrawal_threshold,
        },
        database,
        Arc::clone(&metrics),
        status_tx.clone(),
    );
    let slot_monitor = monitor_rpc_slot(
        config.rpc_http_url,
        config.program_id.clone(),
        status_tx,
        Arc::clone(&metrics),
        config.max_ready_slot_lag,
    );

    tokio::select! {
        result = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()) => result?,
        result = observer.run() => result?,
        () = slot_monitor => {},
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
