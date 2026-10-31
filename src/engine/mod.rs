mod worker;
pub mod files;

#[cfg(windows)]
mod platform_windows;
#[cfg(target_os = "linux")]
mod platform_linux;

use crate::report::TestResult;
use std::io;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

/// Shared metrics collected by all worker threads
pub struct Metrics {
    pub total_ops: AtomicU64,
    pub total_bytes: AtomicU64,
    pub latency_sum_ns: AtomicU64,
    pub latency_samples: AtomicU64,
    /// Sorted latency samples for percentile calculation (collected post-test)
    latency_reservoir: std::sync::Mutex<Vec<u64>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            total_ops: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            latency_sum_ns: AtomicU64::new(0),
            latency_samples: AtomicU64::new(0),
            latency_reservoir: std::sync::Mutex::new(Vec::with_capacity(100_000)),
        }
    }

    pub fn record_latency(&self, ns: u64) {
        self.latency_sum_ns.fetch_add(ns, Ordering::Relaxed);
        self.latency_samples.fetch_add(1, Ordering::Relaxed);
        // Reservoir sampling: keep up to 100k samples
        let mut reservoir = self.latency_reservoir.lock().unwrap();
        if reservoir.len() < 100_000 {
            reservoir.push(ns);
        } else {
            // Random replacement
            let idx = rand::random::<usize>() % reservoir.len();
            reservoir[idx] = ns;
        }
    }

    pub fn percentile(&self, p: f64) -> f64 {
        let mut reservoir = self.latency_reservoir.lock().unwrap();
        if reservoir.is_empty() {
            return 0.0;
        }
        reservoir.sort_unstable();
        let idx = ((p / 100.0) * (reservoir.len() as f64 - 1.0)) as usize;
        reservoir[idx.min(reservoir.len() - 1)] as f64 / 1_000.0 // ns -> us
    }
}

/// What a test runs against: devices (threads per device) or a file set (threads in total)
pub enum Target {
    Devices(Vec<String>),
    Files { paths: Vec<String>, size: u64 },
}

/// Configuration for a benchmark test
pub struct TestConfig<'a> {
    pub target: &'a Target,
    pub io_size: u64,
    pub threads: u32,  // in total, or per device with per_device
    pub per_device: bool,
    pub queue_depth: u32,
    pub duration_secs: u32,
    pub is_write: bool,
}

/// Run a benchmark test and return the result
pub fn run_test(config: &TestConfig) -> io::Result<TestResult> {
    let test_type = if config.is_write { "Write" } else { "Read" };
    let io_kb = config.io_size / 1024;
    let devices = match config.target {
        Target::Devices(paths) => paths.len() as u32,
        Target::Files { .. } => 1,
    };
    let total_threads = if config.per_device { config.threads * devices } else { config.threads };
    let spread = if devices > 1 { format!(" over {} devices", devices) } else { String::new() };

    println!(
        "  {} test: {}KB blocks, {} threads{}, QD={}, {} I/Os in flight, {} seconds",
        test_type,
        io_kb,
        total_threads,
        spread,
        config.queue_depth,
        total_threads * config.queue_depth,
        config.duration_secs
    );

    let metrics = Arc::new(Metrics::new());
    let stop = Arc::new(AtomicBool::new(false));
    let duration = Duration::from_secs(config.duration_secs as u64);

    // what each worker thread gets: one device, or its slice of the files
    let plan: Vec<Vec<(String, u64)>> = match config.target {
        Target::Devices(paths) => {
            if paths.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "No devices specified"));
            }
            if total_threads < devices {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{} threads can't cover {} devices; use at least {}", total_threads, devices, devices),
                ));
            }
            let mut sizes = Vec::new();
            let mut total_size: u64 = 0;
            for path in paths {
                let size = get_device_size(path)?;
                if size == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Device {} size is 0", path),
                    ));
                }
                total_size += size;
                sizes.push(size);
            }
            // thread i goes to device i % n
            let plan: Vec<_> = (0..total_threads as usize)
                .map(|i| vec![(paths[i % paths.len()].clone(), sizes[i % paths.len()])])
                .collect();
            println!(
                "  Total device size: {:.2} GB ({} device{})",
                total_size as f64 / (1024.0 * 1024.0 * 1024.0),
                paths.len(),
                crate::plural(paths.len())
            );
            plan
        }
        Target::Files { paths, size } => {
            let t = total_threads as usize;
            let n = paths.len();
            println!(
                "  {} files x {} KB ({:.2} GB), opening them...",
                n,
                size / 1024,
                (n as u64 * size) as f64 / (1024.0 * 1024.0 * 1024.0)
            );
            // thread i takes files i, i+t, ...; more threads than files share them
            (0..t)
                .map(|i| {
                    if t >= n {
                        vec![(paths[i % n].clone(), *size)]
                    } else {
                        paths.iter().skip(i).step_by(t).map(|p| (p.clone(), *size)).collect()
                    }
                })
                .collect()
        }
    };

    // workers open everything first; the clock starts when all are ready
    let ready = Arc::new(Barrier::new(plan.len() + 1));
    let mut handles = Vec::new();

    for (id, targets) in plan.into_iter().enumerate() {
        let metrics = Arc::clone(&metrics);
        let stop = Arc::clone(&stop);
        let ready = Arc::clone(&ready);
        let io_size = config.io_size;
        let queue_depth = config.queue_depth;
        let is_write = config.is_write;

        let handle = std::thread::spawn(move || {
            if let Err(e) = worker::run_worker(
                id as u32,
                &targets,
                io_size,
                queue_depth,
                is_write,
                &ready,
                &stop,
                &metrics,
            ) {
                eprintln!("  Worker {} error: {}", id, e);
            }
        });
        handles.push(handle);
    }

    ready.wait();
    let start = Instant::now();

    // Progress reporting
    let report_interval = Duration::from_secs(5);
    let mut next_report = start + report_interval;

    while start.elapsed() < duration {
        std::thread::sleep(Duration::from_millis(100));

        if Instant::now() >= next_report {
            let elapsed = start.elapsed().as_secs_f64();
            let ops = metrics.total_ops.load(Ordering::Relaxed) as f64;
            let bytes = metrics.total_bytes.load(Ordering::Relaxed) as f64;
            let mbps = bytes / elapsed / (1024.0 * 1024.0);
            let iops = ops / elapsed;

            let lat_samples = metrics.latency_samples.load(Ordering::Relaxed) as f64;
            let lat_sum = metrics.latency_sum_ns.load(Ordering::Relaxed) as f64;
            let avg_lat_us = if lat_samples > 0.0 {
                lat_sum / lat_samples / 1_000.0
            } else {
                0.0
            };

            println!(
                "  {:>3.0}s: {:>8.2} MB/s | {:>10.0} IOPS | {:>8.1} us avg lat",
                elapsed, mbps, iops, avg_lat_us
            );
            next_report += report_interval;
        }
    }

    // Signal stop
    stop.store(true, Ordering::Release);

    // Wait for workers
    for h in handles {
        let _ = h.join();
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_ops = metrics.total_ops.load(Ordering::Relaxed) as f64;
    let total_bytes = metrics.total_bytes.load(Ordering::Relaxed) as f64;
    let lat_samples = metrics.latency_samples.load(Ordering::Relaxed) as f64;
    let lat_sum = metrics.latency_sum_ns.load(Ordering::Relaxed) as f64;

    let throughput_mbps = total_bytes / elapsed / (1024.0 * 1024.0);
    let iops = total_ops / elapsed;
    let avg_lat_us = if lat_samples > 0.0 {
        lat_sum / lat_samples / 1_000.0
    } else {
        0.0
    };
    let p50_us = metrics.percentile(50.0);
    let p99_us = metrics.percentile(99.0);

    println!(
        "  RESULT: {:.2} MB/s | {:.0} IOPS | avg {:.1} us | p50 {:.1} us | p99 {:.1} us",
        throughput_mbps, iops, avg_lat_us, p50_us, p99_us
    );

    Ok(TestResult {
        throughput_mbps,
        iops,
        latency_avg_us: avg_lat_us,
        latency_p50_us: p50_us,
        latency_p99_us: p99_us,
        threads: total_threads,
        devices,
        queue_depth: config.queue_depth,
        in_flight: total_threads * config.queue_depth,
        block_size_kb: (config.io_size / 1024) as u32,
        duration_secs: config.duration_secs,
    })
}

/// Create a file device of the specified size
pub fn create_file_device(path: &str, size_gb: u64) -> io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let size_bytes = size_gb * 1024 * 1024 * 1024;
    println!("Creating file device: {} ({} GB)", path, size_gb);

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    let chunk_size: usize = 1024 * 1024; // 1 MB chunks
    let mut buf = vec![0u8; chunk_size];
    // Fill with random data
    for chunk in buf.chunks_mut(8) {
        let val = rand::random::<u64>();
        let bytes = val.to_le_bytes();
        let len = chunk.len().min(8);
        chunk[..len].copy_from_slice(&bytes[..len]);
    }

    let total_chunks = size_bytes / chunk_size as u64;
    for i in 0..total_chunks {
        file.write_all(&buf)?;
        if i % 1024 == 0 {
            let pct = (i as f64 / total_chunks as f64) * 100.0;
            print!("\r  Progress: {:.1}%", pct);
        }
    }

    // Write remaining bytes
    let remainder = (size_bytes % chunk_size as u64) as usize;
    if remainder > 0 {
        file.write_all(&buf[..remainder])?;
    }

    println!("\r  Progress: 100.0% - Done!");
    file.flush()?;
    Ok(())
}

/// Prep device by writing random data
pub fn prep_device(path: &str) -> io::Result<()> {
    let size = get_device_size(path)?;
    println!(
        "Preparing device: {} ({:.2} GB)",
        path,
        size as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    let file = open_device_write(path)?;

    let chunk_size: u64 = 4 * 1024 * 1024; // 4MB for better throughput
    let aligned_buf = alloc_aligned(chunk_size as usize, 4096);
    // Fill with random data
    for chunk in unsafe {
        std::slice::from_raw_parts_mut(aligned_buf.ptr, aligned_buf.len)
    }
    .chunks_mut(8)
    {
        let val = rand::random::<u64>();
        let bytes = val.to_le_bytes();
        let len = chunk.len().min(8);
        chunk[..len].copy_from_slice(&bytes[..len]);
    }

    let total_chunks = size / chunk_size;
    let start = Instant::now();

    print!("  Progress:   0.0%");
    let _ = std::io::stdout().flush();

    for i in 0..total_chunks {
        let offset = i * chunk_size;
        write_at_raw(&file, &aligned_buf, offset)?;
        // Report every 256MB (64 x 4MB chunks)
        if i % 64 == 0 {
            let pct = (i as f64 / total_chunks as f64) * 100.0;
            let elapsed = start.elapsed().as_secs_f64();
            let written_mb = (i * chunk_size) as f64 / (1024.0 * 1024.0);
            let mbps = if elapsed > 0.0 { written_mb / elapsed } else { 0.0 };
            print!("\r  Progress: {:>5.1}%  ({:.0} MB/s)", pct, mbps);
            let _ = std::io::stdout().flush();
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_mb = size as f64 / (1024.0 * 1024.0);
    let mbps = if elapsed > 0.0 { total_mb / elapsed } else { 0.0 };
    println!("\r  Progress: 100.0%  ({:.0} MB/s avg) - Done!    ", mbps);
    Ok(())
}

/// Aligned buffer for direct I/O
pub struct AlignedBuf {
    pub ptr: *mut u8,
    pub len: usize,
    layout: std::alloc::Layout,
}

unsafe impl Send for AlignedBuf {}
unsafe impl Sync for AlignedBuf {}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.ptr, self.layout);
        }
    }
}

impl AlignedBuf {
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

/// Allocate a buffer aligned to the specified alignment
pub fn alloc_aligned(size: usize, align: usize) -> AlignedBuf {
    let layout = std::alloc::Layout::from_size_align(size, align).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        panic!("Failed to allocate aligned buffer");
    }
    AlignedBuf { ptr, len: size, layout }
}

// Platform-specific functions - implemented in platform_windows.rs / platform_linux.rs

#[cfg(windows)]
pub use platform_windows::{get_device_size, open_device_write, DeviceHandle, write_at_raw, normalize_device_path};

#[cfg(target_os = "linux")]
pub use platform_linux::{get_device_size, open_device_read, open_device_write, DeviceHandle, read_at_raw, write_at_raw};
