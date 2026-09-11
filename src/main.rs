mod cli;
mod engine;
mod report;

use clap::Parser;
use cli::Args;
use engine::TestConfig;
use report::BenchmarkReport;
use std::path::Path;

/// Parse device argument(s) and normalize Windows paths
fn parse_devices(device_args: Vec<String>) -> Vec<String> {
    let mut devices = Vec::new();

    for arg in device_args {
        // Handle comma-separated values
        for part in arg.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                #[cfg(windows)]
                let normalized = engine::normalize_device_path(trimmed);
                #[cfg(not(windows))]
                let normalized = trimmed.to_string();

                devices.push(normalized);
            }
        }
    }

    if devices.is_empty() {
        eprintln!("Error: No valid devices specified");
        std::process::exit(1);
    }

    devices
}

// run something against every device at once, bail out if any of them fail
fn run_on_all_devices<F>(devices: &[String], what: &str, f: F)
where
    F: Fn(&str) -> std::io::Result<()> + Sync,
{
    let f = &f;
    let failed = std::thread::scope(|s| {
        let handles: Vec<_> = devices
            .iter()
            .map(|d| {
                s.spawn(move || match f(d) {
                    Ok(()) => {
                        println!("  ✓ {}", d);
                        false
                    }
                    Err(e) => {
                        eprintln!("Error {} device {}: {}", what, d, e);
                        true
                    }
                })
            })
            .collect();
        handles.into_iter().any(|h| h.join().unwrap())
    });
    if failed {
        std::process::exit(1);
    }
}

fn main() {
    let args = Args::parse();

    println!("4Corners Disk Benchmark (Rust)");
    println!("==============================");
    println!();

    // Parse and normalize device list
    let devices = parse_devices(args.device);
    let device_display = if devices.len() == 1 {
        devices[0].clone()
    } else {
        format!("{} devices", devices.len())
    };

    // Create file devices if requested (all of them, in parallel)
    if args.create_file {
        println!("Creating {} file device{}...", devices.len(), if devices.len() == 1 { "" } else { "s" });
        let size_gb = args.file_size;
        run_on_all_devices(&devices, "creating", |d| engine::create_file_device(d, size_gb));
        println!("All file devices created successfully");
        println!();
    }

    // Prep device if requested (all devices in parallel)
    if args.prep {
        println!("Preparing {} device{}...", devices.len(), if devices.len() == 1 { "" } else { "s" });
        run_on_all_devices(&devices, "preparing", engine::prep_device);
        println!("All devices prepared successfully");
        println!();
    }

    // Determine which tests to run
    let run_all = args.tests == "all";
    let run_read_tp = run_all || args.tests.contains("read-tp");
    let run_write_tp = run_all || args.tests.contains("write-tp");
    let run_read_iops = run_all || args.tests.contains("read-iops");
    let run_write_iops = run_all || args.tests.contains("write-iops");

    let mut report = BenchmarkReport::new(&device_display);

    println!("Starting benchmark tests...");
    println!();

    // Read Throughput
    if run_read_tp {
        println!("Running Read Throughput Test...");
        let config = TestConfig {
            device_paths: devices.clone(),
            io_size: args.read_tp_bs as u64 * 1024,
            threads: args.read_tp_threads,
            queue_depth: args.read_tp_qd,
            duration_secs: args.duration,
            is_write: false,
        };
        match engine::run_test(&config) {
            Ok(result) => report.read_throughput = Some(result),
            Err(e) => eprintln!("Read throughput error: {}", e),
        }
        println!();
    }

    // Write Throughput
    if run_write_tp {
        println!("Running Write Throughput Test...");
        let config = TestConfig {
            device_paths: devices.clone(),
            io_size: args.write_tp_bs as u64 * 1024,
            threads: args.write_tp_threads,
            queue_depth: args.write_tp_qd,
            duration_secs: args.duration,
            is_write: true,
        };
        match engine::run_test(&config) {
            Ok(result) => report.write_throughput = Some(result),
            Err(e) => eprintln!("Write throughput error: {}", e),
        }
        println!();
    }

    // Read IOPS
    if run_read_iops {
        println!("Running Read IOPS Test...");
        let config = TestConfig {
            device_paths: devices.clone(),
            io_size: args.read_iops_bs as u64 * 1024,
            threads: args.read_iops_threads,
            queue_depth: args.read_iops_qd,
            duration_secs: args.duration,
            is_write: false,
        };
        match engine::run_test(&config) {
            Ok(result) => report.read_iops = Some(result),
            Err(e) => eprintln!("Read IOPS error: {}", e),
        }
        println!();
    }

    // Write IOPS
    if run_write_iops {
        println!("Running Write IOPS Test...");
        let config = TestConfig {
            device_paths: devices.clone(),
            io_size: args.write_iops_bs as u64 * 1024,
            threads: args.write_iops_threads,
            queue_depth: args.write_iops_qd,
            duration_secs: args.duration,
            is_write: true,
        };
        match engine::run_test(&config) {
            Ok(result) => report.write_iops = Some(result),
            Err(e) => eprintln!("Write IOPS error: {}", e),
        }
        println!();
    }

    println!("Benchmark completed!");
    println!();
    println!("{}", report.generate_text_report());

    if let Err(e) = report.save(Path::new(".")) {
        eprintln!("Warning: failed to save reports: {}", e);
    }
}
