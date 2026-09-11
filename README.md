# 4C - Disk Benchmark Tool (Rust Edition)

A high-performance disk I/O benchmark utility that measures the "4 corners" of storage performance using true async I/O on both Windows and Linux.

## Overview

**4C** measures:
- **Read Throughput** — Maximum read bandwidth (large block sequential)
- **Write Throughput** — Maximum write bandwidth (large block sequential)
- **Read IOPS** — Random read operations per second (small block random)
- **Write IOPS** — Random write operations per second (small block random)

## Documentation

- **[CLI-REFERENCE.md](CLI-REFERENCE.md)** — Complete command-line parameter reference
- **[BUILD.md](BUILD.md)** — Detailed build instructions for Windows and Linux, including cross-compilation

## Key Features

### Multi-Device Testing 🆕
- Test multiple devices simultaneously for aggregate performance
- IOPS and throughput summed across all devices
- Latency averaged across all devices
- Windows shorthand: use `4` instead of `\\.\PhysicalDrive4`
- Use case: Saturate storage fabric/HBA when single devices can't max out capacity

```powershell
# Test three drives together (aggregate IOPS/throughput)
4c --device "4,5,6" --duration 60

# Or with full paths
4c --device \\.\PhysicalDrive4 --device \\.\PhysicalDrive5

# Or multiple test files - all get created (and filled with random data)
4c --device "E:\test\file1,E:\test\file2" --create-file --file-size 20
```

### Async I/O
- **Windows**: IOCP-based overlapped I/O with batched completions (`GetQueuedCompletionStatusEx`)
- **Linux**: io_uring-based async I/O (kernel 5.1+)
- Configurable queue depth per test type
- Default IOPS queue depth: 1 per thread (120 concurrent I/Os per device with 120 threads)

### Performance Metrics
- **Throughput** (MB/s) — Data transfer rate
- **IOPS** — Operations per second
- **Latency** — Average, P50, and P99 latencies in microseconds

### File I/O
- `--create-file` — Create a test file device, filled with random data (one per path when multiple are given)
- `--prep` — Pre-condition existing devices/files with random data (all devices, in parallel; skipped when `--create-file` is used)
- Duplicate devices, or raw devices mixed with file paths, are rejected up front
- Direct I/O mode (`O_DIRECT` on Linux, `FILE_FLAG_NO_BUFFERING` on Windows)

### Test Selection
Run all 4 tests or individual tests:
```powershell
# All tests
4c --device \\.\D:

# IOPS only
4c --device \\.\D: --tests read-iops,write-iops

# Read tests only
4c --device \\.\D: --tests read-tp,read-iops
```

### Custom Configuration
```powershell
# High-thread NVMe test
4c --device \\.\PhysicalDrive1 `
  --read-iops-threads 256 --write-iops-threads 256 `
  --read-iops-qd 64 --write-iops-qd 64

# Multi-device (aggregate) test
4c --device "4,5,6" --read-iops-threads 128 --write-iops-threads 128

# Short 30-second test
4c --device \\.\D: --duration 30

# Custom block sizes
4c --device \\.\D: --read-tp-bs 256 --write-tp-bs 128

# File system tests for volumes:
4c --device "E:\test\file1,E:\test\file2" --prep --create-file --file-size 20 
```

## Reporting

Reports are automatically generated in JSON and text format:
- `4c-report-YYYYMMDD-HHMMSS.txt` — Human-readable format
- `4c-report-YYYYMMDD-HHMMSS.json` — Machine-readable format

## Building

See [BUILD.md](BUILD.md) for detailed build instructions.

### Quick Build (Windows)
```powershell
cd C:\Users\Jar\Dropbox\VSCode2\4c
cargo build --release
```
Binary: `target\release\4c.exe` (810 KB)

### Quick Build (Linux)
```bash
cd ~/Dropbox/VSCode2/4c
cargo build --release
```
Binary: `target/release/4c`

### Cross-Compilation (Linux Binary on Windows)

Use WSL2 for the simplest experience:
```powershell
wsl -d Ubuntu  # Or your WSL distro
```
Then inside WSL:
```bash
cd /mnt/c/Users/Jar/Dropbox/VSCode2/4c
cargo build --release
```

## Performance Tuning

### Thread Count
- **Read Throughput**: 30 (default)
- **Write Throughput**: 16 (default)
- **Read IOPS**: 120 (default)
- **Write IOPS**: 120 (default)

Increase for high-performance devices:
- NVMe: 256+ threads for IOPS tests
- SATA SSD: 128 threads for IOPS tests

### Queue Depth
- **Throughput**: 1 (default)
- **IOPS**: 1 (default)

Increase to 4–256 per thread for maximum IOPS on capable devices.

### Block Size
- **Read Throughput**: 128 KB (default)
- **Write Throughput**: 64 KB (default)
- **Read IOPS**: 4 KB (default, industry standard)
- **Write IOPS**: 4 KB (default, industry standard)

## Permissions

- **Windows**: Administrator privileges required for physical drives
- **Linux**: Root/sudo required for block devices

## Safety

⚠️ **Write tests are destructive** — they overwrite data. Use on empty devices or test files only.

Safe testing:
```powershell
# Read-only tests (safe for production)
4c --device \\.\D: --tests read-tp,read-iops

# Test file (always safe)
4c --device C:\test\bench.dat --create-file --file-size 50
```

## Platform Support

| Platform | Support | I/O Method | Kernel Version |
|----------|---------|----------|-----------------|
| Windows | ✅ Full | IOCP | Windows 10+ |
| Linux | ✅ Full | io_uring | 5.1+ |

## Architecture

```
src/
├── main.rs              # Entry point, test orchestration
├── cli.rs               # CLI argument parsing
├── report.rs            # JSON + text report generation
└── engine/
    ├── mod.rs           # Core engine, buffer allocation, file ops
    ├── worker.rs        # Platform-agnostic worker dispatch
    ├── platform_windows.rs  # IOCP implementation
    └── platform_linux.rs    # io_uring implementation
```

## Comparison to Other Tools

| Feature | 4C | fio | vdbench | vdo-simulator |
|---------|-----|-----|---------|---------------|
| **Platform** | Windows, Linux | Linux | Windows, Solaris | Linux |
| **Easy CLI** | ✅ | ❌ Complex | ❌ Complex | ✅ |
| **Async I/O** | ✅ IOCP/io_uring | ✅ io_uring | ✅ Native | ✅ Native |
| **4-corner test** | ✅ Built-in | ❌ Manual | ✅ Built-in | ❌ |
| **JSON output** | ✅ | ✅ | ❌ | ✅ |
| **Single binary** | ✅ | ❌ | ❌ | ✅ |

## Contributing

Found a bug or have a suggestion? Check the source code in `src/` and feel free to file issues.


