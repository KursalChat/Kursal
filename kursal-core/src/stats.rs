use prometheus_client::registry::Registry;
use serde::Serialize;
use std::{
    sync::{Arc, LazyLock, Mutex},
    time::Instant,
};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub type SharedRegistry = Arc<Mutex<Registry>>;

pub fn new_shared_registry() -> SharedRegistry {
    Arc::new(Mutex::new(Registry::default()))
}

static GLOBAL_REGISTRY: LazyLock<SharedRegistry> = LazyLock::new(new_shared_registry);
static GLOBAL_COLLECTOR: LazyLock<Mutex<StatsCollector>> =
    LazyLock::new(|| Mutex::new(StatsCollector::new(global_registry())));

pub fn global_registry() -> SharedRegistry {
    GLOBAL_REGISTRY.clone()
}

pub fn global_sample() -> NodeStats {
    GLOBAL_COLLECTOR
        .lock()
        .map(|mut c| c.sample())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStats {
    pub cpu_percent: f32,
    pub mem_bytes: u64,
    pub uptime_secs: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub rate_in: f64,
    pub rate_out: f64,
}

pub fn parse_bandwidth(text: &str) -> (u64, u64) {
    let mut inbound = 0u64;
    let mut outbound = 0u64;
    for line in text.lines() {
        if !line.starts_with("libp2p_bandwidth_bytes_total{") {
            continue;
        }
        let Some(value) = line.rsplit(' ').find_map(|tok| tok.parse::<u64>().ok()) else {
            continue;
        };
        if line.contains("direction=\"Inbound\"") {
            inbound = inbound.saturating_add(value);
        } else if line.contains("direction=\"Outbound\"") {
            outbound = outbound.saturating_add(value);
        }
    }
    (inbound, outbound)
}

pub struct StatsCollector {
    registry: SharedRegistry,
    system: System,
    pid: Pid,
    start: Instant,
    prev: Option<(u64, u64, Instant)>,
}

impl StatsCollector {
    pub fn new(registry: SharedRegistry) -> Self {
        Self {
            registry,
            system: System::new(),
            pid: sysinfo::get_current_pid().unwrap_or(Pid::from_u32(0)),
            start: Instant::now(),
            prev: None,
        }
    }

    pub fn sample(&mut self) -> NodeStats {
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[self.pid]), true);
        let (cpu_percent, mem_bytes) = self
            .system
            .process(self.pid)
            .map(|p| (p.cpu_usage(), p.memory()))
            .unwrap_or((0.0, 0));

        let mut encoded = String::new();
        if let Ok(registry) = self.registry.lock() {
            let _ = prometheus_client::encoding::text::encode(&mut encoded, &registry);
        }
        let (bytes_in, bytes_out) = parse_bandwidth(&encoded);

        let now = Instant::now();
        let (rate_in, rate_out) = match self.prev {
            Some((prev_in, prev_out, prev_t)) => {
                let dt = now.duration_since(prev_t).as_secs_f64().max(0.001);
                (
                    bytes_in.saturating_sub(prev_in) as f64 / dt,
                    bytes_out.saturating_sub(prev_out) as f64 / dt,
                )
            }
            None => (0.0, 0.0),
        };
        self.prev = Some((bytes_in, bytes_out, now));

        NodeStats {
            cpu_percent,
            mem_bytes,
            uptime_secs: self.start.elapsed().as_secs(),
            bytes_in,
            bytes_out,
            rate_in,
            rate_out,
        }
    }
}
