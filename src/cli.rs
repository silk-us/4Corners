use clap::{ArgGroup, Parser};

#[derive(Parser, Debug, Clone)]
#[command(name = "4c", version, about = "4Corners Disk Benchmark - CLI")]
#[command(group(ArgGroup::new("target").required(true).args(["device", "filepath"])))]
pub struct Args {
    /// Device or file path(s) - can specify multiple times or comma-separated
    /// On Windows: use \\.\PhysicalDrive4 or just 4
    #[arg(short, long)]
    pub device: Vec<String>,

    /// Folder to create a set of test files in (instead of --device)
    #[arg(long, requires_all = ["filecount", "filesizek"], conflicts_with_all = ["create_file", "file_size", "prep"])]
    pub filepath: Option<String>,

    /// Number of test files (with --filepath)
    #[arg(long, requires = "filepath", conflicts_with = "device")]
    pub filecount: Option<u32>,

    /// Size of each test file in KB, a multiple of 4 (with --filepath)
    #[arg(long, requires = "filepath", conflicts_with = "device")]
    pub filesizek: Option<u64>,

    /// Recreate the test files even if a matching set is already there
    #[arg(long, requires = "filepath", conflicts_with = "device")]
    pub recreate: bool,

    /// Delete the test files when the run is done
    #[arg(long, requires = "filepath", conflicts_with = "device")]
    pub cleanup: bool,

    /// Test duration in seconds
    #[arg(long, default_value_t = 30)]
    pub duration: u32,

    /// Read throughput threads, in total. A comma list (120,240,480) runs the test once per count
    #[arg(long, value_delimiter = ',', default_values_t = [120u32], value_parser = clap::value_parser!(u32).range(1..))]
    pub read_tp_threads: Vec<u32>,

    /// Write throughput threads, in total. A comma list runs the test once per count
    #[arg(long, value_delimiter = ',', default_values_t = [64u32], value_parser = clap::value_parser!(u32).range(1..))]
    pub write_tp_threads: Vec<u32>,

    /// Read IOPS threads, in total. A comma list runs the test once per count
    #[arg(long, value_delimiter = ',', default_values_t = [480u32], value_parser = clap::value_parser!(u32).range(1..))]
    pub read_iops_threads: Vec<u32>,

    /// Write IOPS threads, in total. A comma list runs the test once per count
    #[arg(long, value_delimiter = ',', default_values_t = [480u32], value_parser = clap::value_parser!(u32).range(1..))]
    pub write_iops_threads: Vec<u32>,

    /// Thread counts are per device instead of in total (the pre-0.9.1 meaning)
    #[arg(long, conflicts_with = "filepath")]
    pub per_device: bool,

    /// Read throughput queue depth per thread
    #[arg(long, default_value_t = 1)]
    pub read_tp_qd: u32,

    /// Write throughput queue depth per thread
    #[arg(long, default_value_t = 1)]
    pub write_tp_qd: u32,

    /// Read IOPS queue depth per thread
    #[arg(long, default_value_t = 1)]
    pub read_iops_qd: u32,

    /// Write IOPS queue depth per thread
    #[arg(long, default_value_t = 1)]
    pub write_iops_qd: u32,

    /// Read throughput block size (KB)
    #[arg(long, default_value_t = 128)]
    pub read_tp_bs: u32,

    /// Write throughput block size (KB)
    #[arg(long, default_value_t = 64)]
    pub write_tp_bs: u32,

    /// Read IOPS block size (KB)
    #[arg(long, default_value_t = 4)]
    pub read_iops_bs: u32,

    /// Write IOPS block size (KB)
    #[arg(long, default_value_t = 4)]
    pub write_iops_bs: u32,

    /// Prep device before testing (writes random data)
    #[arg(long)]
    pub prep: bool,

    /// Create a file device before testing
    #[arg(long)]
    pub create_file: bool,

    /// File device size in GB (if creating)
    #[arg(long, default_value_t = 10)]
    pub file_size: u64,

    /// Tests to run: all, read-tp, write-tp, read-iops, write-iops (comma-separated)
    #[arg(long, default_value = "all")]
    pub tests: String,
}
