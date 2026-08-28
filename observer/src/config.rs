use {clap::Parser, std::net::SocketAddr};

#[derive(Clone, Debug, Parser)]
#[command(name = "iron-vault-observer", version, about)]
pub struct Config {
    #[arg(
        long,
        env = "IRONVAULT_RPC_HTTP_URL",
        default_value = "http://127.0.0.1:8899"
    )]
    pub rpc_http_url: String,
    #[arg(
        long,
        env = "IRONVAULT_RPC_WS_URL",
        default_value = "ws://127.0.0.1:8900"
    )]
    pub rpc_ws_url: String,
    #[arg(long, env = "IRONVAULT_PROGRAM_ID", default_value = crate::DEFAULT_PROGRAM_ID)]
    pub program_id: String,
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,
    #[arg(long, env = "IRONVAULT_LISTEN_ADDR", default_value = "0.0.0.0:8080")]
    pub listen_addr: SocketAddr,
    #[arg(
        long,
        env = "IRONVAULT_LARGE_WITHDRAWAL_THRESHOLD",
        default_value_t = 50_000
    )]
    pub large_withdrawal_threshold: u64,
    #[arg(long, env = "IRONVAULT_MAX_READY_SLOT_LAG", default_value_t = 150)]
    pub max_ready_slot_lag: u64,
    #[arg(long, env = "IRONVAULT_RECONNECT_DELAY_MS", default_value_t = 1_000)]
    pub reconnect_delay_ms: u64,
}
