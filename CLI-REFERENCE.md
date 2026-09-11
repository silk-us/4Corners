# 4C Disk Benchmark - CLI Reference

## Overview

`4c` is a cross-platform disk I/O benchmark tool that measures the "4 corners" of storage performance:

- **Read Throughput** — Maximum read bandwidth (large block sequential)
- **Write Throughput** — Maximum write bandwidth (large block sequential)
- **Read IOPS** — Random read operations per second (small block random)
- **Write IOPS** — Random write operations per second (small block random)

Each test reports throughput (MB/s), IOPS, and latency (avg, p50, p99).

## Usage

```
4c --device <DEVICE> [--device <DEVICE2> ...] [OPTIONS]
```

## Required

| Option | Description |
|--------|-------------|
| `-d`, `--device <PATH>` | Device or file path to benchmark. Can be specified multiple times or comma-separated for multi-device testing. |

### Windows device paths
```
\\.\PhysicalDrive1       Physical drive (full path)
4                        Physical drive (shorthand - converts to \\.\PhysicalDrive4)
\\.\D:                   Volume
C:\test\benchmark.dat    File
```

**Note:** On Windows, device numbers are automatically converted to full paths. Both `4` and `\\.\PhysicalDrive4` refer to the same device.

### Linux device paths
```
/dev/sdb                 SATA/SAS drive
/dev/nvme0n1             NVMe drive
/mnt/storage/bench.dat   File
```

## Test Selection

| Option | Default | Description |
|--------|---------|-------------|
| `--tests <LIST>` | `all` | Comma-separated list of tests to run |

Values: `all`, `read-tp`, `write-tp`, `read-iops`, `write-iops`

```powershell
# Run all 4 tests
4c --device \\.\D:

# IOPS tests only
4c --device \\.\D: --tests read-iops,write-iops

# Read tests only
4c --device \\.\D: --tests read-tp,read-iops
```

## Duration

| Option | Default | Description |
|--------|---------|-------------|
| `--duration <SECS>` | `30` | Duration of each test in seconds |

## Thread Configuration

Each test type uses its own thread count. More threads generate more concurrent I/O.

**Important:** When testing multiple devices, thread counts are **per device**. Total threads = devices × threads.

| Option | Default | Description |
|--------|---------|-------------|
| `--read-tp-threads` | `30` | Threads per device for read throughput test |
| `--write-tp-threads` | `16` | Threads per device for write throughput test |
| `--read-iops-threads` | `120` | Threads per device for read IOPS test |
| `--write-iops-threads` | `120` | Threads per device for write IOPS test |

### Guidelines by device type

| Device | Throughput Threads | IOPS Threads |
|--------|--------------------|--------------|
| HDD | 8–16 | 32–64 |
| SATA SSD | 16–32 | 64–128 |
| NVMe SSD | 32–64 | 128–256 |
| High-end NVMe | 64–128 | 256–512 |

**Multi-device example:** Testing 3 devices with 120 IOPS threads spawns 360 total threads (120 per device).

## Queue Depth

Queue depth controls how many I/Os each thread keeps in flight simultaneously. Higher queue depth drives more parallelism per thread. This is a key parameter for IOPS performance.

| Option | Default | Description |
|--------|---------|-------------|
| `--read-tp-qd` | `1` | Queue depth per thread for read throughput |
| `--write-tp-qd` | `1` | Queue depth per thread for write throughput |
| `--read-iops-qd` | `1` | Queue depth per thread for read IOPS |
| `--write-iops-qd` | `1` | Queue depth per thread for write IOPS |

Total concurrent I/Os = threads × queue depth × devices. For example, the default IOPS config runs 120 threads × 1 QD × 1 device = 120 concurrent I/Os.

## Block Size

Block size is the amount of data transferred per I/O operation, specified in KB.

| Option | Default | Description |
|--------|---------|-------------|
| `--read-tp-bs` | `128` | Block size (KB) for read throughput |
| `--write-tp-bs` | `64` | Block size (KB) for write throughput |
| `--read-iops-bs` | `4` | Block size (KB) for read IOPS |
| `--write-iops-bs` | `4` | Block size (KB) for write IOPS |

## File & Device Preparation

| Option | Default | Description |
|--------|---------|-------------|
| `--create-file` | off | Create a file device before testing |
| `--file-size <GB>` | `10` | Size of the file to create (in GB) |
| `--prep` | off | Write random data to device before testing |

Use `--create-file` to benchmark against a file instead of a raw device. Use `--prep` to pre-condition a device with random data for accurate first-write performance.

Both apply to every device in the list and run in parallel. Notes:

- `--create-file` already fills each new file with random data, so `--prep` is skipped when both flags are given.
- `--create-file` only works with file paths, not raw devices.
- `--prep` works fine on existing files — handy for re-normalizing a file after a write test.

## Multi-Device Testing

Test multiple devices simultaneously to achieve aggregate performance across devices. Results are combined:

- **IOPS**: Summed across all devices
- **Throughput (MB/s)**: Summed across all devices
- **Latency**: Averaged across all devices

### Specifying Multiple Devices

The device list is checked before anything runs. Listing the same device twice, or mixing raw devices with file paths in one run, is rejected with an error.

**Windows:**
```powershell
# Multiple --device flags
4c --device 4 --device 5 --device 6

# Comma-separated (Windows shorthand)
4c --device "4,5,6"

# Full paths
4c --device \\.\PhysicalDrive4 --device \\.\PhysicalDrive5

# Mixed format
4c --device "4,5" --device \\.\PhysicalDrive6
```

**Linux:**
```bash
# Multiple --device flags
sudo ./4c --device /dev/sdb --device /dev/sdc --device /dev/sdd

# Comma-separated
sudo ./4c --device "/dev/sdb,/dev/sdc,/dev/sdd"

# NVMe devices
sudo ./4c --device /dev/nvme0n1 --device /dev/nvme1n1

# Mixed NVMe and SATA
sudo ./4c --device /dev/sdb --device /dev/nvme0n1
```

### Use Cases

**Saturate storage fabric:** When a single device can't fully load your HBA/NIC/fabric, test multiple devices to measure aggregate capacity.

**Parallel iSCSI targets:** Test multiple LUNs from the same iSCSI target to measure target-level performance.

**RAID array testing:** Test multiple drives in a RAID setup to measure controller throughput.

## Examples

### Quick 30-second test on a volume
```powershell
4c --device \\.\D: --duration 30
```

### Create a 20 GB test file and benchmark it
```powershell
4c --device C:\test\bench.dat --create-file --file-size 20
```

### IOPS-focused test with higher queue depth
```powershell
4c --device \\.\PhysicalDrive1 --tests read-iops,write-iops --read-iops-qd 64 --write-iops-qd 64
```

### High-thread NVMe test
```powershell
4c --device \\.\PhysicalDrive1 --read-iops-threads 256 --write-iops-threads 256
```

### Read-only test (safe for production volumes)
```powershell
4c --device \\.\D: --tests read-tp,read-iops
```

### Prep device then run full benchmark
```powershell
4c --device \\.\PhysicalDrive1 --prep
```

### Linux: NVMe full test
```bash
sudo ./4c --device /dev/nvme0n1 --duration 120
```

### Multi-device: Test three drives together (Windows shorthand)
```powershell
4c --device "4,5,6" --duration 60
```

### Multi-device: IOPS test across multiple iSCSI LUNs (Windows)
```powershell
4c --device 10 --device 11 --device 12 --tests read-iops,write-iops
```

### Multi-device: Saturate fabric with high thread count (Windows)
```powershell
4c --device "4,5,6,7" --read-iops-threads 256 --write-iops-threads 256
```

### Multi-device: Test three drives together (Linux)
```bash
sudo ./4c --device "/dev/sdb,/dev/sdc,/dev/sdd" --duration 60
```

### Multi-device: IOPS test across multiple NVMe drives (Linux)
```bash
sudo ./4c --device /dev/nvme0n1 --device /dev/nvme1n1 --device /dev/nvme2n1 --tests read-iops,write-iops
```

### Multi-device: Prep and benchmark multiple drives in parallel (Linux)
```bash
sudo ./4c --device "/dev/sdb,/dev/sdc,/dev/sdd" --prep --read-iops-threads 256 --write-iops-threads 256
```

### Multi-device: Create two 20 GB test files, then benchmark (Windows)
```powershell
./4c --device "E:\test\file1,E:\test\file2" --create-file --file-size 20
```

### Multi-device: Re-prep existing test files before another run (Windows)
```powershell
./4c --device "E:\test\file1,E:\test\file2" --prep
```

## Output

### Console
Real-time progress is printed every 5 seconds during each test:

**Single device:**
```
Running Read IOPS Test...
  Read test: 4KB blocks, 120 threads per device, QD=32, 60 seconds
  Total device size: 476.94 GB (1 device)
    5s:  1234.56 MB/s |     316045 IOPS |    121.3 us avg lat
   10s:  1245.67 MB/s |     318891 IOPS |    119.8 us avg lat
  ...
  RESULT: 1240.12 MB/s | 317471 IOPS | avg 120.5 us | p50 98.2 us | p99 412.7 us
```

**Multiple devices:**
```
Running Read IOPS Test...
  Read test: 4KB blocks, 120 threads per device, QD=32, 60 seconds
  Total device size: 1430.82 GB (3 devices)
    5s:  3678.90 MB/s |     941800 IOPS |    122.5 us avg lat
   10s:  3701.23 MB/s |     947515 IOPS |    121.1 us avg lat
  ...
  RESULT: 3692.45 MB/s | 945907 IOPS | avg 121.8 us | p50 99.3 us | p99 415.2 us
```

Metrics are aggregated: IOPS and throughput are summed, latency is averaged.

### Report Files
Two files are saved to the current directory after each run:

- `4c-report-YYYYMMDD-HHMMSS.txt` — Human-readable text report
- `4c-report-YYYYMMDD-HHMMSS.json` — Machine-readable JSON report

## Permissions

- **Windows**: Administrator required for raw devices (`\\.\PhysicalDrive#`, `\\.\D:`). Files work as regular user.
- **Linux**: Root/sudo required for block devices (`/dev/sd*`, `/dev/nvme*`). Files work as regular user.

## Safety

Write tests are destructive. They overwrite data on the target device. Do not run write tests against drives containing data you need. Use `--tests read-tp,read-iops` for read-only testing on production systems.

## Building

```
cargo build --release
```

Binary output: `target/release/4c.exe` (Windows) or `target/release/4c` (Linux).
