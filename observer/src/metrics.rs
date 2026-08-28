use std::{
    fmt::Write,
    sync::atomic::{AtomicI64, AtomicU64, Ordering},
};

#[derive(Default)]
pub struct Metrics {
    pub events_total: AtomicU64,
    pub rpc_errors_total: AtomicU64,
    pub rpc_reconnects_total: AtomicU64,
    pub observer_slot_lag: AtomicI64,
    pub decode_errors_total: AtomicU64,
    pub escrows_created_total: AtomicU64,
    pub withdrawals_requested_total: AtomicU64,
    pub withdrawals_executed_total: AtomicU64,
    pub pauses_total: AtomicU64,
}

impl Metrics {
    pub fn render(&self) -> String {
        let mut output = String::new();
        counter(
            &mut output,
            "ironvault_events_total",
            "Persisted protocol events",
            self.events_total.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_rpc_errors_total",
            "Solana RPC and WebSocket errors",
            self.rpc_errors_total.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_rpc_reconnects_total",
            "WebSocket reconnections",
            self.rpc_reconnects_total.load(Ordering::Relaxed),
        );
        gauge(
            &mut output,
            "ironvault_observer_slot_lag",
            "Latest RPC slot minus latest observed slot",
            self.observer_slot_lag.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_decode_errors_total",
            "IronVault event decode errors",
            self.decode_errors_total.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_escrows_created_total",
            "Created escrows",
            self.escrows_created_total.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_withdrawals_requested_total",
            "Requested timelocked withdrawals",
            self.withdrawals_requested_total.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_withdrawals_executed_total",
            "Executed timelocked withdrawals",
            self.withdrawals_executed_total.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "ironvault_pauses_total",
            "Protocol and vault pause activations",
            self.pauses_total.load(Ordering::Relaxed),
        );
        output
    }
}

fn counter(output: &mut String, name: &str, help: &str, value: u64) {
    writeln!(
        output,
        "# HELP {name} {help}\n# TYPE {name} counter\n{name} {value}"
    )
    .expect("writing to String cannot fail");
}

fn gauge(output: &mut String, name: &str, help: &str, value: i64) {
    writeln!(
        output,
        "# HELP {name} {help}\n# TYPE {name} gauge\n{name} {value}"
    )
    .expect("writing to String cannot fail");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_every_required_metric() {
        let rendered = Metrics::default().render();
        for metric in [
            "ironvault_events_total",
            "ironvault_rpc_errors_total",
            "ironvault_rpc_reconnects_total",
            "ironvault_observer_slot_lag",
            "ironvault_decode_errors_total",
            "ironvault_escrows_created_total",
            "ironvault_withdrawals_requested_total",
            "ironvault_withdrawals_executed_total",
            "ironvault_pauses_total",
        ] {
            assert!(rendered.contains(metric), "missing {metric}");
        }
    }
}
