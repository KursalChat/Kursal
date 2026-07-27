use kursal_core::first_contact::otp;
use kursal_core::network::dht::{DHT_LONG_TARGET, mine_pow};
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BenchmarkMode {
    Parallel,
    Sequential,
}

#[derive(Clone, serde::Serialize)]
pub struct BenchmarkMeta {
    pub id: &'static str,
    pub mode: BenchmarkMode,
    pub default_iterations: usize,
}

#[derive(Clone, serde::Serialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub elapsed_ms: u64,
}

#[derive(Clone, serde::Serialize)]
pub struct BenchmarkResult {
    pub iterations: usize,
    pub average_per_iteration_ms: f64,
    pub average_with_threading_ms: f64,
    pub total_ms: f64,
    pub iterations_per_second: f64,
}

trait Benchmark: Sync {
    fn id(&self) -> &'static str;
    fn mode(&self) -> BenchmarkMode;
    fn default_iterations(&self) -> usize;
    fn run_once(&self, index: usize) -> Result<(), String>;
}

struct OtpBench;
impl Benchmark for OtpBench {
    fn id(&self) -> &'static str {
        "otp"
    }
    fn mode(&self) -> BenchmarkMode {
        BenchmarkMode::Parallel
    }
    fn default_iterations(&self) -> usize {
        100
    }
    fn run_once(&self, _index: usize) -> Result<(), String> {
        let otp = otp::generate_otp().map_err(|e| format!("OTP generation failed: {e:?}"))?;
        otp::hash_otp(&otp).map_err(|e| format!("OTP hashing failed: {e:?}"))?;
        Ok(())
    }
}

struct OtpPowBench;
impl Benchmark for OtpPowBench {
    fn id(&self) -> &'static str {
        "otp_pow"
    }
    fn mode(&self) -> BenchmarkMode {
        BenchmarkMode::Sequential
    }
    fn default_iterations(&self) -> usize {
        10
    }
    fn run_once(&self, index: usize) -> Result<(), String> {
        let otp = otp::generate_otp().map_err(|e| format!("OTP generation failed: {e:?}"))?;
        let hash = otp::hash_otp(&otp).map_err(|e| format!("OTP hashing failed: {e:?}"))?;
        let mut message = hash.to_vec();
        message.extend_from_slice(&(index as u64).to_le_bytes());
        mine_pow(&DHT_LONG_TARGET, &message, &[0u8; 32])
            .map_err(|e| format!("PoW mining failed: {e:?}"))?;
        Ok(())
    }
}

struct PowBench;
impl Benchmark for PowBench {
    fn id(&self) -> &'static str {
        "offline"
    }
    fn mode(&self) -> BenchmarkMode {
        BenchmarkMode::Sequential
    }
    fn default_iterations(&self) -> usize {
        10
    }
    fn run_once(&self, index: usize) -> Result<(), String> {
        let mut message = b"kursal-offline-benchmark".to_vec();
        message.extend_from_slice(&(index as u64).to_le_bytes());
        mine_pow(&DHT_LONG_TARGET, &message, &[0u8; 32])
            .map_err(|e| format!("PoW mining failed: {e:?}"))?;
        Ok(())
    }
}

fn registry() -> Vec<Box<dyn Benchmark>> {
    vec![
        Box::new(OtpBench),
        Box::new(OtpPowBench),
        Box::new(PowBench),
    ]
}

static BENCHMARK_CANCELLED: AtomicBool = AtomicBool::new(false);
static BENCHMARK_RUNNING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn list_benchmarks() -> Vec<BenchmarkMeta> {
    registry()
        .iter()
        .map(|b| BenchmarkMeta {
            id: b.id(),
            mode: b.mode(),
            default_iterations: b.default_iterations(),
        })
        .collect()
}

#[tauri::command]
pub fn cancel_benchmark() -> Result<(), String> {
    if !BENCHMARK_RUNNING.load(Ordering::SeqCst) {
        return Err("No benchmark is running".to_string());
    }
    BENCHMARK_CANCELLED.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn is_benchmark_running() -> bool {
    BENCHMARK_RUNNING.load(Ordering::SeqCst)
}

#[tauri::command]
pub async fn run_benchmark(
    app: AppHandle,
    id: String,
    iterations: usize,
) -> Result<BenchmarkResult, String> {
    if iterations == 0 {
        return Err("Iterations must be greater than zero".to_string());
    }
    if BENCHMARK_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Benchmark is already running".to_string());
    }
    BENCHMARK_CANCELLED.store(false, Ordering::SeqCst);

    let outcome = run_inner(app, id, iterations).await;

    BENCHMARK_RUNNING.store(false, Ordering::SeqCst);
    outcome
}

async fn run_inner(
    app: AppHandle,
    id: String,
    iterations: usize,
) -> Result<BenchmarkResult, String> {
    let completed = Arc::new(AtomicUsize::new(0));
    let progress_done = Arc::new(AtomicBool::new(false));
    let progress_handle = spawn_progress(
        app.clone(),
        completed.clone(),
        progress_done.clone(),
        iterations,
    );

    let overall_start = Instant::now();
    let times = tokio::task::spawn_blocking({
        let completed = completed.clone();
        move || run_iterations(&id, iterations, &completed)
    })
    .await
    .map_err(|_| "Benchmark thread panicked".to_string())?;
    let total_time = overall_start.elapsed();

    progress_done.store(true, Ordering::SeqCst);
    let _ = progress_handle.await;

    let _ = app.emit(
        "benchmark-progress",
        BenchmarkProgress {
            current: iterations,
            total: iterations,
            elapsed_ms: u64::try_from(total_time.as_millis()).unwrap_or(u64::MAX),
        },
    );

    summarize(iterations, times?, total_time)
}

fn run_iterations(
    id: &str,
    iterations: usize,
    completed: &AtomicUsize,
) -> Result<Vec<Duration>, String> {
    let registry = registry();
    let bench = registry
        .iter()
        .find(|b| b.id() == id)
        .ok_or_else(|| format!("Unknown benchmark: {id}"))?;

    let run_index = |index: usize| -> Result<Duration, String> {
        if BENCHMARK_CANCELLED.load(Ordering::SeqCst) {
            return Err("Benchmark cancelled".to_string());
        }
        let start = Instant::now();
        if let Err(e) = bench.run_once(index) {
            BENCHMARK_CANCELLED.store(true, Ordering::SeqCst);
            return Err(e);
        }
        let elapsed = start.elapsed();
        completed.fetch_add(1, Ordering::SeqCst);
        Ok(elapsed)
    };

    match bench.mode() {
        BenchmarkMode::Parallel => (0..iterations).into_par_iter().map(run_index).collect(),
        BenchmarkMode::Sequential => (0..iterations).map(run_index).collect(),
    }
}

fn summarize(
    iterations: usize,
    times: Vec<Duration>,
    total_time: Duration,
) -> Result<BenchmarkResult, String> {
    let divisor = u32::try_from(iterations).map_err(|err| err.to_string())?;
    let sum: Duration = times.iter().sum();
    let average_per_iteration = sum / divisor;
    let average_with_threading = total_time / divisor;
    let iterations_per_second = iterations as f64 / total_time.as_secs_f64();

    Ok(BenchmarkResult {
        iterations,
        average_per_iteration_ms: average_per_iteration.as_secs_f64() * 1000.0,
        average_with_threading_ms: average_with_threading.as_secs_f64() * 1000.0,
        total_ms: total_time.as_secs_f64() * 1000.0,
        iterations_per_second,
    })
}

fn spawn_progress(
    app: AppHandle,
    completed: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
    iterations: usize,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let start = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if done.load(Ordering::SeqCst) {
                break;
            }
            let current = completed.load(Ordering::SeqCst);
            let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            let _ = app.emit(
                "benchmark-progress",
                BenchmarkProgress {
                    current,
                    total: iterations,
                    elapsed_ms,
                },
            );
            if current >= iterations {
                break;
            }
        }
    })
}
