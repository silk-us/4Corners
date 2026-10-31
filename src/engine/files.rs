// --filepath mode: a set of equal sized test files under <filepath>/4c-files,
// 1000 per folder, with a manifest so the next run can reuse them
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub struct FileSet {
    pub dir: PathBuf,
    pub count: u32,
    pub size: u64,
}

impl FileSet {
    pub fn new(filepath: &str, count: u32, size_kb: u64) -> Self {
        Self {
            dir: Path::new(filepath).join("4c-files"),
            count,
            size: size_kb * 1024,
        }
    }

    pub fn path(&self, i: u32) -> String {
        self.dir
            .join(format!("d{:03}", i / 1000))
            .join(format!("f{:06}.dat", i))
            .to_string_lossy()
            .into_owned()
    }

    pub fn paths(&self) -> Vec<String> {
        (0..self.count).map(|i| self.path(i)).collect()
    }

    fn manifest(&self) -> PathBuf {
        self.dir.join("manifest.json")
    }

    // bytes taken by whatever set is there now (0 if none)
    pub fn existing_bytes(&self) -> u64 {
        let Ok(text) = fs::read_to_string(self.manifest()) else { return 0 };
        let Ok(m) = serde_json::from_str::<serde_json::Value>(&text) else { return 0 };
        m["count"].as_u64().unwrap_or(0) * m["size"].as_u64().unwrap_or(0)
    }

    // manifest matches and every file is there at the right size
    pub fn is_ready(&self) -> bool {
        let Ok(text) = fs::read_to_string(self.manifest()) else { return false };
        let Ok(m) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
        if m["count"].as_u64() != Some(self.count as u64) || m["size"].as_u64() != Some(self.size) {
            return false;
        }
        // stat them in parallel, one at a time is slow over NFS/SMB
        let next = AtomicU32::new(0);
        let bad = AtomicBool::new(false);
        std::thread::scope(|s| {
            for _ in 0..self.count.min(32) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= self.count || bad.load(Ordering::Relaxed) {
                        return;
                    }
                    if !fs::metadata(self.path(i)).map(|m| m.len() == self.size).unwrap_or(false) {
                        bad.store(true, Ordering::Relaxed);
                    }
                });
            }
        });
        !bad.load(Ordering::Relaxed)
    }

    pub fn create(&self) -> io::Result<()> {
        if self.dir.exists() {
            println!("  Removing the old set...");
            fs::remove_dir_all(&self.dir)?;
        }
        for d in 0..=(self.count - 1) / 1000 {
            fs::create_dir_all(self.dir.join(format!("d{:03}", d)))?;
        }

        let chunk = self.size.min(1 << 20) as usize;
        let mut buf = vec![0u8; chunk];
        for c in buf.chunks_mut(8) {
            let b = rand::random::<u64>().to_le_bytes();
            let n = c.len();
            c.copy_from_slice(&b[..n]);
        }

        let next = AtomicU32::new(0);
        let done = AtomicU32::new(0);
        let failed = AtomicBool::new(false);
        let start = Instant::now();

        let result = std::thread::scope(|s| -> io::Result<()> {
            let workers: Vec<_> = (0..self.count.min(64))
                .map(|_| {
                    s.spawn(|| -> io::Result<()> {
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            if i >= self.count || failed.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            let r = (|| {
                                let mut f = fs::File::create(self.path(i))?;
                                let mut left = self.size;
                                while left > 0 {
                                    let n = left.min(chunk as u64) as usize;
                                    f.write_all(&buf[..n])?;
                                    left -= n as u64;
                                }
                                Ok(())
                            })();
                            if r.is_err() {
                                failed.store(true, Ordering::Relaxed);
                                return r;
                            }
                            done.fetch_add(1, Ordering::Relaxed);
                        }
                    })
                })
                .collect();

            while workers.iter().any(|w| !w.is_finished()) {
                std::thread::sleep(Duration::from_millis(500));
                let n = done.load(Ordering::Relaxed);
                print!(
                    "\r  Created {} of {} files ({:.1}%)",
                    n,
                    self.count,
                    n as f64 * 100.0 / self.count as f64
                );
                let _ = io::stdout().flush();
            }
            for w in workers {
                w.join().unwrap()?;
            }
            Ok(())
        });
        println!();
        result?;

        let secs = start.elapsed().as_secs_f64();
        let mb = (self.count as u64 * self.size) as f64 / (1024.0 * 1024.0);
        println!(
            "  Created {} files in {:.1}s: {:.0} files/s, {:.1} MB/s",
            self.count,
            secs,
            self.count as f64 / secs,
            mb / secs
        );

        // written last, so a cut off create is never reused
        fs::write(
            self.manifest(),
            serde_json::json!({ "count": self.count, "size": self.size }).to_string(),
        )
    }

    pub fn remove(&self) -> io::Result<()> {
        fs::remove_dir_all(&self.dir)
    }
}

#[cfg(target_os = "linux")]
pub fn free_bytes(path: &str) -> io::Result<u64> {
    let c = std::ffi::CString::new(path).unwrap();
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(st.f_bavail as u64 * st.f_frsize as u64)
}

#[cfg(windows)]
pub fn free_bytes(path: &str) -> io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
    let mut avail: u64 = 0;
    if unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(avail)
}

// every file stays open during a test, so lift the soft limit to the hard one
#[cfg(target_os = "linux")]
pub fn raise_open_files() -> u64 {
    unsafe {
        let mut r = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) != 0 {
            return 1024;
        }
        r.rlim_cur = r.rlim_max;
        libc::setrlimit(libc::RLIMIT_NOFILE, &r);
        libc::getrlimit(libc::RLIMIT_NOFILE, &mut r);
        r.rlim_cur as u64
    }
}

// no per process handle limit worth checking on windows
#[cfg(windows)]
pub fn raise_open_files() -> u64 {
    u64::MAX
}
