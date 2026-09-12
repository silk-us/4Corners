# 4C — Four Corners Disk Benchmark

A small, single-binary disk benchmark for Windows and Linux. It measures the four numbers most people actually care about:

| Test | What it does |
|------|--------------|
| Read throughput | Large-block random reads (MB/s) |
| Write throughput | Large-block random writes (MB/s) |
| Read IOPS | Small-block random reads (ops/s) |
| Write IOPS | Small-block random writes (ops/s) |

Every test reports throughput, IOPS, and average / P50 / P99 latency. I/O is direct and async — IOCP on Windows, io_uring on Linux — so the numbers reflect the device, not the OS cache.

## Quick start

```powershell
# Windows: physical drive 4, full run with defaults (30s per test)
4c --device 4

# Windows: a volume, read tests only (safe on live data)
4c --device \\.\D: --tests read-tp,read-iops
```

```bash
# Linux
sudo ./4c --device /dev/nvme0n1 --duration 60
```

Reports are written to the current directory as `4c-report-<timestamp>.txt` and `.json`.

Administrator (Windows) or root (Linux) is required for raw devices.

## Multiple devices

Pass several devices to load them all at once. IOPS and throughput are summed, latency is averaged. Useful for saturating an HBA or storage fabric that a single LUN can't max out.

```powershell
4c --device "4,5,6"
4c --device \\.\PhysicalDrive4 --device \\.\PhysicalDrive5
```

Listing the same device twice, or mixing raw devices with file paths, is rejected.

## Test files

If you'd rather not point the tool at a raw disk, have it create files instead:

```powershell
4c --device "E:\test\file1,E:\test\file2" --create-file --file-size 20
```

New files are filled with random data. To re-condition existing files before another run, use `--prep`.

## Tuning

Threads, queue depth, and block size can be set per test. Defaults are sane for SATA/SAS; NVMe usually wants more:

```powershell
4c --device 1 --read-iops-threads 256 --write-iops-threads 256 --read-iops-qd 32 --write-iops-qd 32
```

See [CLI-REFERENCE.md](CLI-REFERENCE.md) for every option.

## Warning

Write tests and `--prep` overwrite whatever is on the device. Use empty disks or test files.

## Building

`cargo build --release` on either platform. See [BUILD.md](BUILD.md) for prerequisites and cross-compiling.

## Requirements

- Windows 10 or later
- Linux kernel 5.1 or later (io_uring)
