use {
    crate::{metrics::Metrics, rpc::SyncStatus},
    axum::{
        extract::State,
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
        Json, Router,
    },
    serde_json::json,
    sqlx::PgPool,
    std::sync::Arc,
    tokio::sync::watch,
};

#[derive(Clone)]
pub struct HealthState {
    pub database: PgPool,
    pub metrics: Arc<Metrics>,
    pub sync_status: watch::Receiver<SyncStatus>,
    pub max_ready_slot_lag: u64,
}

pub fn router(state: HealthState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "alive"})))
}

async fn readyz(State(state): State<HealthState>) -> Response {
    let sync = state.sync_status.borrow().clone();
    let database_ready = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.database)
        .await
        .is_ok();
    let lag = sync.rpc_slot.saturating_sub(sync.observed_slot);
    let ready =
        database_ready && sync.connected && sync.reconciled && lag <= state.max_ready_slot_lag;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "status": if ready { "ready" } else { "not_ready" },
            "database": database_ready,
            "websocket": sync.connected,
            "reconciled": sync.reconciled,
            "observed_slot": sync.observed_slot,
            "rpc_slot": sync.rpc_slot,
            "slot_lag": lag,
        })),
    )
        .into_response()
}

async fn metrics(State(state): State<HealthState>) -> Response {
    let rendered = state.metrics.render();
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        rendered,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_lag_metric_is_signed_and_observable() {
        let metrics = Metrics::default();
        metrics
            .observer_slot_lag
            .store(7, std::sync::atomic::Ordering::Relaxed);
        assert!(metrics.render().contains("ironvault_observer_slot_lag 7"));
    }
}
