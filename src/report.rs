use chrono::{DateTime, Local};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub throughput_mbps: f64,
    pub iops: f64,
    pub latency_avg_us: f64,
    pub latency_p50_us: f64,
    pub latency_p99_us: f64,
    pub threads: u32,
    pub devices: u32,
    pub queue_depth: u32,
    pub in_flight: u32,
    pub block_size_kb: u32,
    pub duration_secs: u32,
}

// one entry per thread count; a comma list on --*-threads gives several
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub test_date: DateTime<Local>,
    pub device: String,
    pub read_throughput: Vec<TestResult>,
    pub write_throughput: Vec<TestResult>,
    pub read_iops: Vec<TestResult>,
    pub write_iops: Vec<TestResult>,
}

impl BenchmarkReport {
    pub fn new(device: &str) -> Self {
        Self {
            test_date: Local::now(),
            device: device.to_string(),
            read_throughput: Vec::new(),
            write_throughput: Vec::new(),
            read_iops: Vec::new(),
            write_iops: Vec::new(),
        }
    }

    pub fn generate_text_report(&self) -> String {
        let mut s = String::new();
        s.push_str("========================================\n");
        s.push_str("4Corners Disk Benchmark Report\n");
        s.push_str("========================================\n\n");
        s.push_str(&format!(
            "Test Date: {}\n",
            self.test_date.format("%Y-%m-%d %H:%M:%S")
        ));
        s.push_str(&format!("Device: {}\n\n", self.device));

        for (name, results) in [
            ("Read Throughput Test", &self.read_throughput),
            ("Write Throughput Test", &self.write_throughput),
            ("Read IOPS Test", &self.read_iops),
            ("Write IOPS Test", &self.write_iops),
        ] {
            match results.len() {
                0 => {}
                1 => {
                    s.push_str(&format!("{}:\n", name));
                    format_result(&mut s, &results[0]);
                }
                _ => {
                    s.push_str(&format!("{} (thread sweep):\n", name));
                    format_sweep(&mut s, results);
                }
            }
        }

        s.push_str("========================================\n");
        s
    }

    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let timestamp = self.test_date.format("%Y%m%d-%H%M%S");

        let text_path = dir.join(format!("4c-report-{}.txt", timestamp));
        fs::write(&text_path, self.generate_text_report())?;
        println!("Text report saved: {}", text_path.display());

        let json_path = dir.join(format!("4c-report-{}.json", timestamp));
        let json = serde_json::to_string_pretty(self).unwrap();
        fs::write(&json_path, json)?;
        println!("JSON report saved: {}", json_path.display());

        Ok(())
    }
}

fn threads_text(r: &TestResult) -> String {
    if r.devices > 1 {
        format!("{} (spread over {} devices)", r.threads, r.devices)
    } else {
        r.threads.to_string()
    }
}

fn format_result(s: &mut String, r: &TestResult) {
    s.push_str(&format!("  Threads:         {}\n", threads_text(r)));
    s.push_str(&format!("  Queue Depth:     {}\n", r.queue_depth));
    s.push_str(&format!("  I/Os in Flight:  {}\n", r.in_flight));
    s.push_str(&format!("  Block Size:      {} KB\n", r.block_size_kb));
    s.push_str(&format!("  Duration:        {} seconds\n", r.duration_secs));
    s.push_str(&format!("  Throughput:    {:>10.2} MB/s\n", r.throughput_mbps));
    s.push_str(&format!("  IOPS:          {:>10.0}\n", r.iops));
    s.push_str(&format!(
        "  Avg Latency:   {:>10.2} us\n",
        r.latency_avg_us
    ));
    s.push_str(&format!(
        "  P50 Latency:   {:>10.2} us\n",
        r.latency_p50_us
    ));
    s.push_str(&format!(
        "  P99 Latency:   {:>10.2} us\n",
        r.latency_p99_us
    ));
    s.push('\n');
}

// the knee: first step where IOPS grew under 30% while p50 rose over 50%
fn knee(results: &[TestResult]) -> Option<usize> {
    (1..results.len()).find(|&i| {
        let (a, b) = (&results[i - 1], &results[i]);
        a.iops > 0.0
            && a.latency_p50_us > 0.0
            && b.iops / a.iops < 1.30
            && b.latency_p50_us / a.latency_p50_us > 1.50
    })
}

fn format_sweep(s: &mut String, results: &[TestResult]) {
    let r0 = &results[0];
    s.push_str(&format!(
        "  Queue Depth: {}   Block Size: {} KB   Duration: {} seconds each\n\n",
        r0.queue_depth, r0.block_size_kb, r0.duration_secs
    ));
    s.push_str("   Threads  In Flight        MB/s        IOPS     Avg us     P50 us     P99 us\n");
    let k = knee(results);
    for (i, r) in results.iter().enumerate() {
        s.push_str(&format!(
            "  {:>8} {:>10} {:>11.2} {:>11.0} {:>10.1} {:>10.1} {:>10.1}{}\n",
            r.threads,
            r.in_flight,
            r.throughput_mbps,
            r.iops,
            r.latency_avg_us,
            r.latency_p50_us,
            r.latency_p99_us,
            if k == Some(i) { "  <- knee: latency up, IOPS flat" } else { "" }
        ));
    }
    if r0.devices > 1 {
        s.push_str(&format!("  Threads are spread over {} devices\n", r0.devices));
    }
    s.push('\n');
}
