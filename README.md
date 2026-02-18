# Game Mods

**Monorepo for game modification projects built on the Injection Toolkit framework.**

Game-specific mods that inject custom rendering, overlays, and automation into game processes. Built in Rust with cross-platform IPC, shared memory, and Vulkan/OpenVR hooking.

All code is authored by AI agents under human direction.

## Projects

| Mod | Game | Description |
|-----|------|-------------|
| [NMS Cockpit Video](projects/nms-cockpit-video/) | No Man's Sky | In-cockpit video player via Vulkan injection + desktop overlay |

## Architecture

The injection toolkit provides a **minimal injection, maximal external processing** framework:

```
Launcher (orchestration)
  |
  +-- Daemon (external process)     <-- video decode, audio, IPC server
  |     |
  |     +-- Shared Memory           <-- lock-free frame transport (seqlock)
  |     +-- IPC (named pipes)       <-- commands, projection data
  |
  +-- Injector (DLL in target)      <-- Vulkan hooks, texture rendering
  |
  +-- Overlay (optional desktop)    <-- egui + wgpu transparent window
```

### Core Libraries

| Crate | Purpose |
|-------|---------|
| `itk-protocol` | Wire protocol definitions (serde + bincode) |
| `itk-shmem` | Cross-platform shared memory (Windows/Linux) |
| `itk-ipc` | Cross-platform IPC channels (named pipes/Unix sockets) |
| `itk-sync` | Clock synchronization and drift correction |
| `itk-video` | Video decoding via ffmpeg + frame management |
| `itk-net` | P2P networking for multiplayer sync (laminar) |

### Framework Templates

| Crate | Purpose |
|-------|---------|
| `itk-daemon` | Central coordinator daemon template |
| `itk-overlay` | wgpu-based transparent overlay window template |
| `itk-native-dll` | Windows DLL injection template |
| `itk-ld-preload` | Linux LD_PRELOAD injection template |

### Tools

| Tool | Purpose |
|------|---------|
| `mem-scanner` | Memory pattern scanning utility for reverse engineering |

## Project Structure

```
game-mods/
+-- core/                          # Shared libraries
|   +-- itk-protocol/
|   +-- itk-shmem/
|   +-- itk-ipc/
|   +-- itk-sync/
|   +-- itk-video/
|   +-- itk-net/
+-- daemon/                        # Framework: coordinator daemon
+-- overlay/                       # Framework: transparent overlay
+-- injectors/                     # Framework: injection templates
|   +-- windows/native-dll/
|   +-- linux/ld-preload/
+-- projects/                      # Game-specific mods
|   +-- nms-cockpit-video/
|       +-- daemon/                # NMS video playback daemon
|       +-- injector/              # Vulkan DLL injection (cdylib)
|       +-- overlay/               # Desktop overlay (egui + wgpu)
|       +-- launcher/              # Process orchestrator
|       +-- mod/                   # Reloaded-II C# mod (optional)
|       +-- docs/                  # Reverse engineering notes
+-- tools/
|   +-- mem-scanner/               # Memory scanning utility
+-- docker/                        # CI Dockerfiles
+-- .github/                       # GitHub Actions workflows
```

## Development

```bash
# Containerized CI (matches GitHub Actions)
docker compose --profile ci run --rm rust-ci cargo fmt --all -- --check
docker compose --profile ci run --rm rust-ci cargo clippy --all-targets -- -D warnings
docker compose --profile ci run --rm rust-ci cargo test
docker compose --profile ci run --rm rust-ci cargo build --release
docker compose --profile ci run --rm rust-ci cargo deny check
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (Edition 2024) |
| Video | ffmpeg-next 7.0, cpal 0.15 (audio) |
| Graphics | ash 0.38 (Vulkan), wgpu 0.20, egui 0.28 |
| Hooking | retour 0.3 (function detours) |
| Platform | windows 0.58 (Win32), nix 0.29 (Unix) |
| CI/CD | GitHub Actions (self-hosted runner, Docker containers) |

## License

Dual-licensed under [Unlicense](LICENSE) and [MIT](LICENSE-MIT).
