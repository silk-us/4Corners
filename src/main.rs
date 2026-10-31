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

fn fail(msg: &str) -> ! {
    eprintln!("Error: {}", msg);
    std::process::exit(1);
}

fn main() {
    let args = Args::parse();

    println!("4Corners Disk Benchmark (Rust)");
    println!("==============================");
    println!();

    // Determine which tests to run
    let run_all = args.tests == "all";
    let run_read_tp = run_all || args.tests.contains("read-tp");
    let run_write_tp = run_all || args.tests.contains("write-tp");
    let run_read_iops = run_all || args.tests.contains("read-iops");
    let run_write_iops = run_all || args.tests.contains("write-iops");

    let mut file_set: Option<engine::files::FileSet> = None;
    let (target, display) = match &args.filepath {
        Some(filepath) => {
            let count = args.filecount.unwrap();
            let size_kb = args.filesizek.unwrap();
            if count == 0 {
                fail("--filecount must be at least 1");
            }
            if size_kb == 0 || size_kb % 4 != 0 {
                fail("--filesizek must be a multiple of 4 (direct I/O needs 4 KB alignment)");
            }
            if !Path::new(filepath).is_dir() {
                fail(&format!("--filepath {} is not a folder", filepath));
            }
            // a block bigger than a file has nowhere to go
            for (on, name, bs) in [
                (run_read_tp, "read-tp", args.read_tp_bs),
                (run_write_tp, "write-tp", args.write_tp_bs),
                (run_read_iops, "read-iops", args.read_iops_bs),
                (run_write_iops, "write-iops", args.write_iops_bs),
            ] {
                if on && bs as u64 > size_kb {
                    fail(&format!(
                        "--filesizek {} is smaller than the {} block size ({} KB); pick tests with --tests or lower --{}-bs",
                        size_kb, name, bs, name
                    ));
                }
            }

            let threads = [
                (run_read_tp, &args.read_tp_threads),
                (run_write_tp, &args.write_tp_threads),
                (run_read_iops, &args.read_iops_threads),
                (run_write_iops, &args.write_iops_threads),
            ]
            .iter()
            .filter(|(on, _)| *on)
            .flat_map(|(_, t)| t.iter().map(|&n| n as u64))
            .max()
            .unwrap_or(1);
            let limit = engine::files::raise_open_files();
            let need = count as u64 + threads + 64;
            if limit < need {
                fail(&format!(
                    "{} files stay open during a test but the open file limit is {}; raise it (ulimit -n {}) or use fewer files",
                    count, limit, need
                ));
            }

            let set = engine::files::FileSet::new(filepath, count, size_kb);
            println!("Test files: {} x {} KB in {}", count, size_kb, set.dir.display());
            if !args.recreate && set.is_ready() {
                println!("  Reusing the existing set (--recreate to make new ones)");
            } else {
                let need = count as u64 * set.size;
                match engine::files::free_bytes(filepath) {
                    Ok(free) if free + set.existing_bytes() < need => fail(&format!(
                        "the set needs {:.2} GB but only {:.2} GB is free in {}",
                        need as f64 / 1e9,
                        free as f64 / 1e9,
                        filepath
                    )),
                    Err(e) => eprintln!("  Warning: could not check free space: {}", e),
                    _ => {}
                }
                if let Err(e) = set.create() {
                    fail(&format!("creating the test files failed: {}", e));
                }
            }
            println!();

            let target = engine::Target::Files { paths: set.paths(), size: set.size };
            let display = format!("{} ({} files x {} KB)", filepath, count, size_kb);
            file_set = Some(set);
            (target, display)
        }
        None => {
            // Parse and normalize device list
            let devices = match parse_devices(args.device.clone()) {
                Ok(d) => d,
                Err(e) => fail(&e),
            };
            let display = if devices.len() == 1 {
                devices[0].clone()
            } else {
                format!("{} devices", devices.len())
            };

            // Create file devices if requested (all of them, in parallel)
            if args.create_file {
                if devices.iter().any(|d| is_raw_device(d)) {
                    fail("--create-file only works with file paths, not raw devices");
                }
                println!("Creating {} file device{} ({} GB each)...", devices.len(), plural(devices.len()), args.file_size);
                if run_on_all_devices(&devices, |d| engine::create_file_device(d, args.file_size)).is_err() {
                    fail("file creation failed");
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
                    fail("device prep failed");
                }
                println!("All devices prepared successfully");
                println!();
            }

            (engine::Target::Devices(devices), display)
        }
    };

    let mut report = BenchmarkReport::new(&display);

    println!("Starting benchmark tests...");
    println!();

    // each test runs once per thread count given (a comma list sweeps it)
    let series = |name: &str, threads: &[u32], qd: u32, bs_kb: u32, is_write: bool| {
        println!("Running {} Test...", name);
        let mut results = Vec::new();
        for (i, &t) in threads.iter().enumerate() {
            if threads.len() > 1 {
                println!("  [{}/{}] {} threads", i + 1, threads.len(), t);
            }
            let config = TestConfig {
                target: &target,
                io_size: bs_kb as u64 * 1024,
                threads: t,
                per_device: args.per_device,
                queue_depth: qd,
                duration_secs: args.duration,
                is_write,
            };
            match engine::run_test(&config) {
                Ok(result) => results.push(result),
                Err(e) => eprintln!("{} error: {}", name, e),
            }
        }
        println!();
        results
    };

    if run_read_tp {
        report.read_throughput = series("Read Throughput", &args.read_tp_threads, args.read_tp_qd, args.read_tp_bs, false);
    }
    if run_write_tp {
        report.write_throughput = series("Write Throughput", &args.write_tp_threads, args.write_tp_qd, args.write_tp_bs, true);
    }
    if run_read_iops {
        report.read_iops = series("Read IOPS", &args.read_iops_threads, args.read_iops_qd, args.read_iops_bs, false);
    }
    if run_write_iops {
        report.write_iops = series("Write IOPS", &args.write_iops_threads, args.write_iops_qd, args.write_iops_bs, true);
    }

    println!("Benchmark completed!");
    println!();
    println!("{}", report.generate_text_report());

    if let Err(e) = report.save(Path::new(".")) {
        eprintln!("Warning: failed to save reports: {}", e);
    }

    if let (true, Some(set)) = (args.cleanup, &file_set) {
        match set.remove() {
            Ok(()) => println!("Removed {}", set.dir.display()),
            Err(e) => eprintln!("Warning: could not remove {}: {}", set.dir.display(), e),
        }
    }
}
