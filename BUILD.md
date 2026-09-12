# Building 4C

## Prerequisites

- Rust, via [rustup](https://rustup.rs/)
- **Windows:** Visual Studio Build Tools with the "Desktop development with C++" workload (the MSVC linker)
- **Linux:** a C compiler (`build-essential`, `gcc`, `base-devel`, etc.)

## Build

```bash
cargo build --release
```

Binary lands at `target\release\4c.exe` on Windows or `target/release/4c` on Linux. Use release builds for benchmarking — debug builds are noticeably slower.

The Windows build links the C runtime statically (see `.cargo/config.toml`), so the exe runs without the VC++ redistributable.

## Static Linux binary

To get a binary that runs on any distro without glibc version issues, build against musl:

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

Binary: `target/x86_64-unknown-linux-musl/release/4c`

## Building the Linux binary from Windows or macOS

Easiest is WSL2 (Windows) — install Rust inside the distro and run the commands above.

Otherwise, use Docker:

```bash
docker run --rm --platform linux/amd64 -v "$PWD:/work" -w /work rust:1-alpine \
  sh -c 'apk add --no-cache musl-dev && cargo build --release --target x86_64-unknown-linux-musl'
```

## Building the Windows binary from Linux or macOS

```bash
docker run --rm -v "$PWD:/work" -w /work ghcr.io/rust-cross/cargo-xwin \
  cargo xwin build --release --target x86_64-pc-windows-msvc
```

Binary: `target/x86_64-pc-windows-msvc/release/4c.exe`

## Troubleshooting

- **`link.exe not found` / MSVC errors on Windows** — install the Build Tools workload above, then open a fresh terminal.
- **`cargo: command not found`** — Rust isn't on your PATH; restart the terminal after installing rustup.
- **Permission denied running the binary on Linux** — `chmod +x target/release/4c`.
- **Weird results or errors opening a device** — you probably aren't running as admin/root.
