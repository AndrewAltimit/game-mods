# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust monorepo for game modification projects built on the **Injection Toolkit (ITK)** framework. Follows a "minimal injection, maximal external processing" architecture where injected code in the target game process is kept minimal, with heavy processing delegated to external daemon and overlay processes communicating via IPC and shared memory.

All code is authored by AI agents under human direction. No external contributions are accepted.

## Build & CI Commands

All CI runs inside Docker containers via `docker compose --profile ci`. The local equivalents:

```bash
# Format check
cargo fmt --all -- --check

# Lint (warnings are errors in CI)
cargo clippy --all-targets -- -D warnings

# Test
cargo test

# Build release
cargo build --release

# License/advisory check
cargo deny check

# Run a single test
cargo test -p itk-protocol -- test_name

# Containerized (matches CI exactly)
docker compose --profile ci run --rm rust-ci cargo test
```

CI pipeline order: fmt check → clippy → test → build → cargo-deny.

## Workspace Structure

**Core libraries** (`core/`): Shared ITK crates used by all projects.
- `itk-protocol` — Wire protocol (20-byte header + bincode payload, CRC32 validated)
- `itk-shmem` — Cross-platform shared memory with seqlock (single-writer, multi-reader)
- `itk-ipc` — Named pipes (Windows) / Unix sockets (Linux)
- `itk-sync` — Clock synchronization and drift correction
- `itk-video` — Video decoding via ffmpeg-next
- `itk-net` — P2P networking via laminar

**Framework templates**: `daemon/`, `overlay/`, `injectors/windows/native-dll/`, `injectors/linux/ld-preload/`

**Active project**: `projects/nms-cockpit-video/` (daemon, injector, overlay, launcher)

**Tools**: `tools/mem-scanner/` — Memory pattern scanning for reverse engineering

## Architecture

```
Launcher (orchestration)
  ├── Daemon (external)     — video decode, audio, IPC server, shared memory writer
  ├── Injector (DLL/SO)     — Vulkan hooks, minimal state extraction, IPC client
  └── Overlay (optional)    — egui + wgpu transparent window, shared memory reader
```

Key design constraints:
- Injector must stay under 5 MB memory, no blocking operations, no complex processing
- Seqlock shared memory is **single-writer only** — multiple writers corrupt data
- All data from injectors is treated as **untrusted** — validate NaN/Inf, bounds, string lengths (256 byte cap), data size (64 KB cap)
- Components run in separate processes; crashes are isolated (overlay crash doesn't crash target)

## Code Conventions

- Rust Edition 2024, max line width 100 chars, 4-space indentation (see `rustfmt.toml`)
- Workspace-level dependency versions in root `Cargo.toml`
- Workspace lints: `clippy::dbg_macro`, `clippy::todo`, `clippy::unimplemented` are warnings; `unsafe_op_in_unsafe_fn` is a warning
- Error handling: `thiserror` for structured errors, `anyhow` for application-level
- Logging: `tracing` + `tracing-subscriber` with env-filter
- Platform abstraction via `cfg_if!` blocks in shmem and IPC crates
- Unsafe code requires detailed safety comments

## Known Advisory Exemptions

See `deny.toml` — two advisories are currently ignored:
- `RUSTSEC-2025-0141` (bincode unmaintained) — migration planned
- `RUSTSEC-2026-0007` (bytes integer overflow) — update pending
