# AGENTS.md

This file provides guidance to AI coding agents working with code in this repository.

## Project Overview

Rust monorepo for game modification projects built on the **Injection Toolkit (ITK)** framework. The architecture follows "minimal injection, maximal external processing" — injected code in the target game process is kept minimal, with heavy processing in external daemon and overlay processes connected via IPC and shared memory.

All code is authored by AI agents under human direction. No external contributions are accepted (see `CONTRIBUTING.md`).

## Build and Test Commands

This is a Cargo workspace. All CI runs containerized via Docker but the commands work locally:

```bash
# Format check
cargo fmt --all -- --check

# Lint (warnings are errors in CI)
cargo clippy --all-targets -- -D warnings

# Run all tests
cargo test

# Run a single crate's tests
cargo test -p itk-protocol

# Run a specific test by name
cargo test -p itk-shmem -- seqlock_concurrent

# Build release
cargo build --release

# License and advisory audit
cargo deny check
```

CI pipeline order: **fmt → clippy → test → build → cargo-deny**.

To match CI exactly (containerized):
```bash
docker compose --profile ci run --rm rust-ci cargo test
```

## Code Style

- Rust Edition 2024. Formatting enforced by `rustfmt.toml`: 100-char max line width, 4-space indentation, Unix newlines, `Tall` fn params layout.
- Run `cargo fmt --all` before committing. CI rejects unformatted code.
- Clippy warnings treated as errors in CI: `cargo clippy --all-targets -- -D warnings`.
- Workspace-level lints in root `Cargo.toml`: `clippy::dbg_macro`, `clippy::todo`, `clippy::unimplemented`, and `unsafe_op_in_unsafe_fn` are all warnings.

## Workspace Structure

13 crates organized into four layers:

**Core libraries** (`core/`):
- `itk-protocol` — Wire protocol (20-byte header + bincode payload, CRC32 validated, 1 MB max)
- `itk-shmem` — Cross-platform shared memory with seqlock (single-writer, multi-reader)
- `itk-ipc` — Named pipes (Windows) / Unix sockets (Linux)
- `itk-sync` — Clock synchronization and drift correction
- `itk-video` — Video decoding via ffmpeg-next
- `itk-net` — P2P networking via laminar

**Framework templates**: `daemon/`, `overlay/`, `injectors/windows/native-dll/`, `injectors/linux/ld-preload/`

**Active project**: `projects/nms-cockpit-video/` — No Man's Sky cockpit video player (daemon, injector, overlay, launcher)

**Tools**: `tools/mem-scanner/` — Memory pattern scanning for reverse engineering

## Architecture

```
Launcher (orchestration)
  ├── Daemon (external)     — video decode, audio, IPC server, shared memory writer
  ├── Injector (DLL/SO)     — Vulkan hooks, minimal state extraction, IPC client
  └── Overlay (optional)    — egui + wgpu transparent window, shared memory reader
```

Components run in separate processes. A crash in any one component does not bring down the others.

## Security Considerations

- All data from injectors is **untrusted**. Validate: NaN/Inf, numeric bounds, string lengths (256-byte cap), data sizes (64 KB cap).
- Seqlock shared memory is **single-writer only**. Multiple concurrent writers corrupt the sequence counter.
- Unsafe code requires detailed `// SAFETY:` comments explaining the invariant.
- Named pipes use process-token ACLs (Windows); Unix sockets use `0600` permissions (Linux).

## Conventions

- Dependencies are managed at workspace level in the root `Cargo.toml`. Add new dependencies there.
- Error handling: `thiserror` for library error types, `anyhow` for application-level errors.
- Logging: `tracing` crate with `tracing-subscriber` env-filter. Not `log` or `println!`.
- Platform-specific code uses `cfg_if!` blocks in `itk-shmem` and `itk-ipc`.
- Protocol changes: add variant to `MessageType` enum in `itk-protocol`, define payload struct with serde derives, update daemon handlers.
- New injector platforms go in `injectors/` and implement IPC client + platform-specific init.

## CI/CD Pipeline

Three GitHub Actions workflows on self-hosted runners:

- **`ci.yml`** — Runs on push to main. Format, lint, test, build, cargo-deny.
- **`main-ci.yml`** — Runs on main push and tags. Same CI stages plus auto-creates GitHub issues on failure.
- **`pr-validation.yml`** — Runs on PRs. CI stages + Gemini AI review (primary) + automated agent fix iterations (max 5, extendable with `[CONTINUE]` comment). Add `no-auto-fix` label to disable automated fixes. *(Note: Codex AI review has been disabled due to OpenAI security concerns — see README.md.)*

Agent commit authors: `AI Review Agent`, `AI Pipeline Agent`, `AI Agent Bot`.

## Known Advisory Exemptions

Two advisories ignored in `deny.toml`:
- `RUSTSEC-2025-0141` — bincode unmaintained (migration planned)
- `RUSTSEC-2026-0007` — bytes integer overflow (update pending)
