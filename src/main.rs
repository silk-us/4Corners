mod cli;
mod engine;
mod report;

use clap::Parser;
use cli::Args;
use engine::TestConfig;
use report::BenchmarkReport;
use std::path::Path;

pub fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// raw disk/volume vs a plain file
fn is_raw_device(path: &str) -> bool {
    path.starts_with(r"\\.\") || path.starts_with("/dev/")
}

/// Parse device argument(s), normalize Windows paths, sanity check the list
fn parse_devices(device_args: Vec<String>) -> Result<Vec<String>, String> {
    let mut devices: Vec<String> = Vec::new();

    for arg in device_args {
        // Handle comma-separated values
        for part in arg.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            #[cfg(windows)]
            let normalized = engine::normalize_device_path(trimmed);
            #[cfg(not(windows))]
            let normalized = trimmed.to_string();

            // same device twice is almost always a typo, dont guess
            if devices.iter().any(|d| d.eq_ignore_ascii_case(&normalized)) {
                return Err(format!("device {} is listed more than once", normalized));
            }
            devices.push(normalized);
        }
    }

    if devices.is_empty() {
        return Err("no valid devices specified".to_string());
    }

    // raw devices and files never mix
    let raw = devices.iter().filter(|d| is_raw_device(d)).count();
    if raw > 0 && raw < devices.len() {
        return Err("raw devices and file paths cannot be mixed in the same run".to_string());
    }

    Ok(devices)
}

// run something against every device at once, first error wins
fn run_on_all_devices<F>(devices: &[String], f: F) -> std::io::Result<()>
where
    F: Fn(&str) -> std::io::Result<()> + Sync,
{
    let f = &f;
    std::thread::scope(|s| {
        let handles: Vec<_> = devices
            .iter()
            .map(|d| {
                s.spawn(move || {
                    let result = f(d);
                    match &result {
                        Ok(()) => println!("  ✓ {}", d),
                        Err(e) => eprintln!("  ✗ {}: {}", d, e),
                    }
                    result
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

fn main() {
    let args = Args::parse();

    println!("4Corners Disk Benchmark (Rust)");
    println!("==============================");
    println!();

    // Parse and normalize device list
    let devices = match parse_devices(args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let device_display = if devices.len() == 1 {
        devices[0].clone()
    } else {
        format!("{} devices", devices.len())
    };

    // Create file devices if requested (all of them, in parallel)
    if args.create_file {
        if devices.iter().any(|d| is_raw_device(d)) {
            eprintln!("Error: --create-file only works with file paths, not raw devices");
            std::process::exit(1);
        }
        println!("Creating {} file device{} ({} GB each)...", devices.len(), plural(devices.len()), args.file_size);
        if run_on_all_devices(&devices, |d| engine::create_file_device(d, args.file_size)).is_err() {
            eprintln!("Error: file creation failed");
            std::process::exit(1);
        }
        println!("All file devices created successfully");
        println!();
    }

    // Prep devices if requested (all devices in parallel)
    // new files are already full of random data so prep is pointless right after create
    if args.prep && args.create_file {
        println!("Skipping --prep: --create-file already fills the files with random data");
        println!();
    } else if args.prep {
        println!("Preparing {} device{}...", devices.len(), plural(devices.len()));
        if run_on_all_devices(&devices, engine::prep_device).is_err() {
            eprintln!("Error: device prep failed");
            std::process::exit(1);
        }
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
