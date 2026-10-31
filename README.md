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

Thread counts are the total for the run, spread over the devices: the default 480 IOPS threads on four devices is 120 each. `--per-device` restores the pre-0.9.1 meaning (threads per device).

## Test files

If you'd rather not point the tool at a raw disk, have it create files instead:

```powershell
4c --device "E:\test\file1,E:\test\file2" --create-file --file-size 20
```

New files are filled with random data. To re-condition existing files before another run, use `--prep`.

## Many small files

To benchmark a share or folder full of files instead of one device, use `--filepath` in place of `--device`. 4c creates the set, then spreads every test's I/O over all the files:

```bash
# 32,000 x 4 KB files on an NFS mount, IOPS tests only
./4c --filepath /mnt/test01 --filecount 32000 --filesizek 4 --tests read-iops,write-iops
```

```powershell
# Windows, an SMB share
4c --filepath \\fileserver\share\bench --filecount 10000 --filesizek 64
```

The set goes in `<filepath>/4c-files` and is reused on the next run with the same count and size, so you only wait for creation once. `--recreate` makes a new set, and `--cleanup` deletes it at the end. Thread counts are for the whole run, and files stay open while a test runs, so the numbers are I/O, not opens. Details: [CLI-REFERENCE.md](CLI-REFERENCE.md#file-set-mode).

## Tuning

Threads, queue depth, and block size can be set per test. Defaults are sane for SATA/SAS; NVMe usually wants more:

```powershell
4c --device 1 --read-iops-threads 256 --write-iops-threads 256 --read-iops-qd 32 --write-iops-qd 32
```

A comma list on any `--*-threads` option sweeps that test, one run per count, and the report marks where latency starts climbing without more IOPS:

```powershell
4c --device "4,5,6,7" --tests read-iops --read-iops-threads 120,240,480,960
```

See [CLI-REFERENCE.md](CLI-REFERENCE.md) for every option.

## Warning

Write tests and `--prep` overwrite whatever is on the device. Use empty disks or test files.

## Building

`cargo build --release` on either platform. See [BUILD.md](BUILD.md) for prerequisites and cross-compiling.

## Requirements

- Windows 10 or later
- Linux kernel 5.1 or later (io_uring)
