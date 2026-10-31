use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Barrier;

use super::Metrics;

/// Main worker entry point - dispatches to platform-specific async I/O.
/// targets: (path, size) this thread spreads its I/O over, one device or a slice of files.
/// The worker opens them all, waits on `ready`, then runs until `stop`.
pub fn run_worker(
    _thread_id: u32,
    targets: &[(String, u64)],
    io_size: u64,
    queue_depth: u32,
    is_write: bool,
    ready: &Barrier,
    stop: &AtomicBool,
    metrics: &Metrics,
) -> io::Result<()> {
    #[cfg(windows)]
    {
        super::platform_windows::worker_iocp(
            targets, io_size, queue_depth, is_write, ready, stop, metrics,
        )
    }

    #[cfg(target_os = "linux")]
    {
        super::platform_linux::worker_io_uring(
            targets, io_size, queue_depth, is_write, ready, stop, metrics,
        )
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        ready.wait();
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Platform not supported",
        ))
    }
}
