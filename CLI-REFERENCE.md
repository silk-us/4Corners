# 4C Disk Benchmark - CLI Reference

4c measures the four corners of storage performance on one or more devices, or on a set of many files:

| Test | `--tests` name | Default block | Default threads | What it measures |
|------|----------------|---------------|-----------------|------------------|
| Read throughput | `read-tp` | 128 KB | 120 | Large-block random reads (MB/s) |
| Write throughput | `write-tp` | 64 KB | 64 | Large-block random writes (MB/s) |
| Read IOPS | `read-iops` | 4 KB | 480 | Small-block random reads (ops/s) |
| Write IOPS | `write-iops` | 4 KB | 480 | Small-block random writes (ops/s) |

Every test reports MB/s, IOPS and latency (average, p50, p99). I/O is direct (no OS cache) and asynchronous: IOCP on Windows, io_uring on Linux.

## Usage

```
4c --device <PATH>[,<PATH>...] [OPTIONS]
4c --filepath <FOLDER> --filecount <N> --filesizek <KB> [OPTIONS]
```

Give exactly one of `--device` or `--filepath`. 4c refuses both together, and refuses neither.

| Target | Option | Use it for |
|--------|--------|------------|
| Devices | `-d`, `--device <PATH>` | Raw disks, volumes, LUNs, or a few large test files. Repeat the option or separate paths with commas |
| File set | `--filepath <FOLDER>` | Many files in a folder, for example an NFS or SMB share. Needs `--filecount` and `--filesizek`. See [File-set mode](#file-set-mode) |

## Threads and concurrency

Load comes from threads. Each thread keeps `QD` I/Os in flight, so the total load is:

```
I/Os in flight = threads x queue depth
```

**Thread counts are the total for the run** in every mode. With four devices and `--read-iops-threads 480`, each device gets 120 threads (thread `i` goes to device `i % devices`). With a file set, the threads share the files. If there are fewer threads than devices, 4c stops with an error rather than leave a device idle.

| Option | Default | Description |
|--------|---------|-------------|
| `--read-tp-threads <N[,N...]>` | `120` | Threads for read throughput |
| `--write-tp-threads <N[,N...]>` | `64` | Threads for write throughput |
| `--read-iops-threads <N[,N...]>` | `480` | Threads for read IOPS |
| `--write-iops-threads <N[,N...]>` | `480` | Threads for write IOPS |
| `--per-device` | off | Treat the thread counts as per device, the meaning before 0.9.1. Device mode only |

The defaults are sized for four devices, which is how 4c is usually run. With one device, the same totals all go to that device.

### Thread sweeps

Give a comma list to run that test once per thread count, in order:

```bash
4c --filepath /mnt/test01 --filecount 32000 --filesizek 4 --tests read-iops,write-iops \
   --read-iops-threads 120,240,480,960 --write-iops-threads 120,240,480,960
```

Each test sweeps its own list; a test given one number runs once. The report shows a table per swept test and marks the **knee**: the first step where IOPS grew less than 30% while p50 latency rose more than 50%. Past the knee, more threads only add queueing. The step before it is the useful number.

### Queue depth

| Option | Default | Description |
|--------|---------|-------------|
| `--read-tp-qd <N>` | `1` | I/Os each read throughput thread keeps in flight |
| `--write-tp-qd <N>` | `1` | Same, write throughput |
| `--read-iops-qd <N>` | `1` | Same, read IOPS |
| `--write-iops-qd <N>` | `1` | Same, write IOPS |

QD 1 with many threads is the default and matches how most applications behave. Raising QD adds load without adding threads, at the cost of latency.

## Block size and duration

| Option | Default | Description |
|--------|---------|-------------|
| `--read-tp-bs <KB>` | `128` | Block size for read throughput |
| `--write-tp-bs <KB>` | `64` | Block size for write throughput |
| `--read-iops-bs <KB>` | `4` | Block size for read IOPS |
| `--write-iops-bs <KB>` | `4` | Block size for write IOPS |
| `--duration <SECS>` | `30` | Length of each test (each step of a sweep) |
| `--tests <LIST>` | `all` | Comma list of `read-tp`, `write-tp`, `read-iops`, `write-iops`, or `all` |

## Device mode

### Paths

| Windows | Linux | What |
|---------|-------|------|
| `4` or `\\.\PhysicalDrive4` | `/dev/sdb`, `/dev/nvme0n1` | Physical disk |
| `\\.\D:` | | Volume |
| `C:\test\bench.dat` | `/mnt/storage/bench.dat` | File |

On Windows a bare number is turned into `\\.\PhysicalDriveN`. Listing a device twice, or mixing raw devices with files in one run, is refused.

With several devices the results are combined: IOPS and MB/s are summed, latency is over all I/Os.

### Test files and prep

| Option | Default | Description |
|--------|---------|-------------|
| `--create-file` | off | Create each `--device` path as a file, filled with random data |
| `--file-size <GB>` | `10` | Size of each created file |
| `--prep` | off | Write random data over each device before testing |

Both run on every device in parallel. `--prep` is skipped after `--create-file`, which already writes random data. `--create-file` only works on file paths.

## File-set mode

Benchmarks many files instead of a few devices.

| Option | Default | Description |
|--------|---------|-------------|
| `--filepath <FOLDER>` | | Folder the test files go in. Must exist |
| `--filecount <N>` | | Number of files (required) |
| `--filesizek <KB>` | | Size of each file in KB, a multiple of 4 (required) |
| `--recreate` | off | Make a new set even if a matching one is there |
| `--cleanup` | off | Delete the set when the run is done |

- **Layout.** Files go in `<filepath>/4c-files/d000/f000000.dat`, 1,000 per folder, filled with random data. Creation runs 64 files at a time and reports files/s and MB/s.
- **Reuse.** A `manifest.json` records the count and size. The next run with the same `--filecount` and `--filesizek` reuses the set, so creation happens once. `--recreate` forces a new set.
- **I/O pattern.** Thread `i` takes files `i`, `i + threads`, and so on (with more threads than files, threads share files). Each I/O picks a random file from the thread's share, then a random block inside it. With `--filesizek 4` and 4 KB blocks, every I/O is a whole file.
- **Opens are not timed.** Every thread opens its files before the clock starts and keeps them open, so results measure I/O, not file opens.
- **Checks before anything runs.** Each selected test's block size must fit in a file: `--filesizek 4` with the default read-tp (128 KB) stops with an error, so use `--tests read-iops,write-iops` or a smaller `--read-tp-bs`. Free space is checked before creating. On Linux, 4c raises its open-file limit and stops with a message if it's still too low.
- `--create-file`, `--file-size`, `--prep` and `--per-device` are device options and can't be used with `--filepath`.

## Examples

```powershell
# Windows: physical disk 4, all four tests with the defaults
4c --device 4

# Four iSCSI LUNs, IOPS only, sweep the load to find the knee
4c --device "10,11,12,13" --tests read-iops,write-iops --read-iops-threads 240,480,960 --write-iops-threads 240,480,960

# Read-only run on a live volume
4c --device \\.\D: --tests read-tp,read-iops

# Two 20 GB test files, created first
4c --device "E:\test\file1,E:\test\file2" --create-file --file-size 20

# SMB share, 10,000 x 64 KB files
4c --filepath \\fileserver\share\bench --filecount 10000 --filesizek 64
```

```bash
# Linux: NVMe, 2 minutes per test
sudo ./4c --device /dev/nvme0n1 --duration 120

# Four 10 GB files on an NFS mount, old per-device thread meaning (120 x 4 = 480 IOPS threads)
./4c --device "/mnt/test01/f1,/mnt/test01/f2,/mnt/test01/f3,/mnt/test01/f4" --read-iops-threads 120 --write-iops-threads 120 --per-device

# 32,000 x 4 KB files on an NFS mount, IOPS sweep
./4c --filepath /mnt/test01 --filecount 32000 --filesizek 4 --tests read-iops,write-iops \
     --read-iops-threads 120,240,480,960 --write-iops-threads 120,240,480,960

# 4,000 x 4 MB files, all four tests, delete them afterwards
./4c --filepath /mnt/test01 --filecount 4000 --filesizek 4096 --cleanup
```

## Output

### Console

Progress every 5 seconds, then one RESULT line per test (per step of a sweep):

```
Running Read IOPS Test...
  Read test: 4KB blocks, 480 threads over 4 devices, QD=1, 480 I/Os in flight, 30 seconds
  Total device size: 40.00 GB (4 devices)
    5s:   612.40 MB/s |     156774 IOPS |   3061.2 us avg lat
  ...
  RESULT: 625.41 MB/s | 160105 IOPS | avg 2899.0 us | p50 1749.1 us | p99 13661.9 us
```

### Report files

Each run writes `4c-report-YYYYMMDD-HHMMSS.txt` and `.json` to the current folder.

A test run once is reported as before, with threads, queue depth and I/Os in flight. A swept test gets a table:

```
Read IOPS Test (thread sweep):
  Queue Depth: 1   Block Size: 4 KB   Duration: 30 seconds each

   Threads  In Flight        MB/s        IOPS     Avg us     P50 us     P99 us
       120        120      265.73       68028     1640.7     1448.8     4944.8
       240        240         ...
```

In the JSON, `read_throughput`, `write_throughput`, `read_iops` and `write_iops` are arrays with one entry per thread count (one entry for a single run). Each entry has `threads` (total), `devices`, `queue_depth`, `in_flight`, `block_size_kb`, `duration_secs`, `throughput_mbps`, `iops` and the three latencies in microseconds. Before 0.9.1 these were single objects.

## Permissions and safety

- Raw devices need Administrator (Windows) or root (Linux). Files and file sets work as a regular user.
- Write tests and `--prep` overwrite the target. Use empty disks, test files or a file set, or run `--tests read-tp,read-iops` on live data.

## Building

See [BUILD.md](BUILD.md). The signed Windows build is `./Build-Signed.ps1`.
