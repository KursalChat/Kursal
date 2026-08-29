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

#[cfg(target_os = "macos")]
mod app_mem {
    use std::time::{Duration, Instant};

    const PROC_PIDCOALITIONINFO: libc::c_int = 20;
    const RESCAN_EVERY: Duration = Duration::from_secs(10);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CoalitionInfo {
        coalition_id: [u64; 2],
        reserved: [u64; 3],
    }

    fn coalition_of(pid: i32) -> Option<u64> {
        let mut info = CoalitionInfo::default();
        let size = libc::c_int::try_from(std::mem::size_of::<CoalitionInfo>()).ok()?;
        let n = unsafe {
            libc::proc_pidinfo(
                pid,
                PROC_PIDCOALITIONINFO,
                0,
                (&mut info as *mut CoalitionInfo).cast(),
                size,
            )
        };
        (n == size).then_some(info.coalition_id[0])
    }

    fn footprint_of(pid: i32) -> Option<u64> {
        let mut ru = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
        let rc =
            unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, ru.as_mut_ptr().cast()) };
        (rc == 0).then(|| unsafe { ru.assume_init() }.ri_phys_footprint)
    }

    fn is_webkit_helper(pid: i32) -> bool {
        const NEEDLE: &[u8] = b"com.apple.WebKit.";

        let mut buf = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let Ok(size) = u32::try_from(buf.len()) else {
            return false;
        };
        let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), size) };
        let Ok(len) = usize::try_from(n) else {
            return false;
        };
        buf[..len].windows(NEEDLE.len()).any(|w| w == NEEDLE)
    }

    fn members_of(id: u64) -> Vec<i32> {
        let cap = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if cap <= 0 {
            return Vec::new();
        }
        let mut pids = vec![0i32; cap as usize];
        let Ok(size) = libc::c_int::try_from(pids.len() * std::mem::size_of::<i32>()) else {
            return Vec::new();
        };
        let bytes = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
        if bytes <= 0 {
            return Vec::new();
        }
        pids.truncate(bytes as usize / std::mem::size_of::<i32>());
        pids.retain(|&pid| pid != 0 && is_webkit_helper(pid) && coalition_of(pid) == Some(id));
        pids
    }

    pub struct AppMem {
        pid: i32,
        coalition: Option<u64>,
        helpers: Vec<i32>,
        last_scan: Option<Instant>,
    }

    impl AppMem {
        pub fn new(pid: u32) -> Self {
            let pid = pid as i32;
            Self {
                pid,
                coalition: coalition_of(pid),
                helpers: Vec::new(),
                last_scan: None,
            }
        }

        pub fn sample(&mut self) -> (u64, u64) {
            let own = footprint_of(self.pid).unwrap_or(0);
            let Some(id) = self.coalition else {
                return (own, 0);
            };

            if self.last_scan.is_none_or(|at| at.elapsed() >= RESCAN_EVERY) {
                self.helpers = members_of(id);
                self.helpers.retain(|&pid| pid != self.pid);
                self.last_scan = Some(Instant::now());
            }

            let mut helpers = 0u64;
            let mut lost = false;
            for &pid in &self.helpers {
                match footprint_of(pid) {
                    Some(bytes) => helpers = helpers.saturating_add(bytes),
                    None => lost = true,
                }
            }
            if lost {
                self.last_scan = None;
            }

            (own, helpers)
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod app_mem {
    use std::collections::HashSet;
    use std::time::{Duration, Instant};
    use sysinfo::{Pid, ProcessesToUpdate, System};

    const RESCAN_EVERY: Duration = Duration::from_secs(10);

    pub struct AppMem {
        pid: Pid,
        system: System,
        tracked: Vec<Pid>,
        last_scan: Option<Instant>,
    }

    impl AppMem {
        pub fn new(pid: u32) -> Self {
            let pid = Pid::from_u32(pid);
            Self {
                pid,
                system: System::new(),
                tracked: vec![pid],
                last_scan: None,
            }
        }

        fn rescan(&mut self) {
            self.system.refresh_processes(ProcessesToUpdate::All, true);

            let mut tree = HashSet::from([self.pid]);
            loop {
                let before = tree.len();
                for (pid, process) in self.system.processes() {
                    if process
                        .parent()
                        .is_some_and(|parent| tree.contains(&parent))
                    {
                        tree.insert(*pid);
                    }
                }
                if tree.len() == before {
                    break;
                }
            }

            self.tracked = tree.into_iter().collect();
            self.last_scan = Some(Instant::now());
        }

        pub fn sample(&mut self) -> (u64, u64) {
            if self.last_scan.is_none_or(|at| at.elapsed() >= RESCAN_EVERY) {
                self.rescan();
            } else {
                self.system
                    .refresh_processes(ProcessesToUpdate::Some(&self.tracked), true);
            }

            let mut own = 0u64;
            let mut helpers = 0u64;
            let mut lost = false;
            for &pid in &self.tracked {
                match self.system.process(pid) {
                    Some(process) if pid == self.pid => own = process.memory(),
                    Some(process) => helpers = helpers.saturating_add(process.memory()),
                    None => lost = true,
                }
            }
            if lost {
                self.last_scan = None;
            }

            (own, helpers)
        }
    }
}

pub struct StatsCollector {
    registry: SharedRegistry,
    system: System,
    pid: Pid,
    mem: app_mem::AppMem,
    start: Instant,
    prev: Option<(u64, u64, Instant)>,
    encoded: String,
}

impl StatsCollector {
    pub fn new(registry: SharedRegistry) -> Self {
        let pid = sysinfo::get_current_pid().unwrap_or(Pid::from_u32(0));
        Self {
            registry,
            system: System::new(),
            pid,
            mem: app_mem::AppMem::new(pid.as_u32()),
            start: Instant::now(),
            prev: None,
            encoded: String::new(),
        }
    }

    pub fn sample(&mut self) -> NodeStats {
        let (cpu_percent, mem_bytes) = if cfg!(any(target_os = "android", target_os = "ios")) {
            (0.0, 0)
        } else {
            self.system
                .refresh_processes(ProcessesToUpdate::Some(&[self.pid]), true);
            let cpu = self
                .system
                .process(self.pid)
                .map(|p| p.cpu_usage())
                .unwrap_or(0.0);
            let (own_bytes, webview_bytes) = self.mem.sample();
            (cpu, own_bytes.saturating_add(webview_bytes))
        };

        self.encoded.clear();
        if let Ok(registry) = self.registry.lock() {
            let _ = prometheus_client::encoding::text::encode(&mut self.encoded, &registry);
        }
        let (bytes_in, bytes_out) = parse_bandwidth(&self.encoded);

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
