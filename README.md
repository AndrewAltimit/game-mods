# Injection Toolkit (ITK) & Game Mods

A Rust framework for safe game modification. Injects minimal code into game processes while delegating heavy processing to external daemons and overlays, connected via IPC and shared memory. All code is authored by AI agents under human direction.

**Use this repo to learn how to:**
- Hook Vulkan and OpenVR pipelines for in-game rendering (including VR)
- Build cross-platform IPC (named pipes / Unix sockets) and lock-free shared memory (seqlock)
- Decode and stream video into a live game process with hardware acceleration
- Structure a Rust injection toolkit as a reusable framework with game-specific project crates
- Run containerized CI/CD for agent-authored Rust code on self-hosted runners

> **OpenAI Notice:** All OpenAI/Codex/GPT integrations have been disabled due to concerns about mass surveillance partnerships. Anthropic Claude is the primary AI backend. See the [template repo](https://github.com/AndrewAltimit/template-repo#ai-agents) for full context.

---

## Quick Start

**Prerequisites:** Linux/Windows with Docker (v20.10+) and Rust toolchain

```bash
git clone https://github.com/AndrewAltimit/game-mods
cd game-mods

# Run the CI pipeline locally (matches GitHub Actions exactly)
docker compose --profile ci run --rm rust-ci cargo fmt --all -- --check
docker compose --profile ci run --rm rust-ci cargo clippy --all-targets -- -D warnings
docker compose --profile ci run --rm rust-ci cargo test
docker compose --profile ci run --rm rust-ci cargo build --release
docker compose --profile ci run --rm rust-ci cargo deny check

# Or run directly (requires local Rust toolchain)
cargo test
cargo build --release
```

CI pipeline order: **fmt check -> clippy -> test -> build -> cargo-deny**

## Projects

| Mod | Game | Description | Docs |
|-----|------|-------------|------|
| [NMS Cockpit Video](projects/nms-cockpit-video/) | No Man's Sky | In-cockpit video player via Vulkan injection + desktop overlay | [README](projects/nms-cockpit-video/README.md) |

## Architecture

The framework follows a **minimal injection, maximal external processing** philosophy. Each component runs in its own process -- a crash in one never brings down the others.

```
Launcher (orchestration)
  ├── Daemon (external process)     video decode, audio, IPC server, shared memory writer
  │     ├── Shared Memory           lock-free frame transport (seqlock, single-writer)
  │     └── IPC (pipes/sockets)     commands, state data, projection matrices
  ├── Injector (DLL/SO in target)   Vulkan hooks, minimal state extraction, IPC client
  └── Overlay (optional desktop)    egui + wgpu transparent window, shared memory reader
```

**Design constraints:**
- Injector stays under 5 MB memory, no blocking operations, no complex processing
- Seqlock shared memory is single-writer only -- multiple writers corrupt data
- All injector data is treated as untrusted (NaN/Inf rejection, bounds checking, size caps)
- Components communicate exclusively via IPC and shared memory, never direct calls

For the full technical design (wire protocol, seqlock algorithm, security threat model, performance budgets), see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Workspace Structure

### Core Libraries (`core/`)

Shared ITK crates used by all projects.

| Crate | Purpose |
|-------|---------|
| `itk-protocol` | Wire protocol -- 20-byte header + bitcode payload, CRC32 validated, 1 MB max |
| `itk-shmem` | Cross-platform shared memory with seqlock (single-writer, multi-reader) |
| `itk-ipc` | Named pipes (Windows) / Unix domain sockets (Linux) |
| `itk-sync` | Clock synchronization and drift correction |
| `itk-video` | Video decoding via ffmpeg-next with D3D11VA hardware acceleration |
| `itk-net` | P2P networking via laminar (UDP broadcast discovery, leader election) |

### Framework Templates

Reusable starting points for new game mods.

| Crate | Purpose |
|-------|---------|
| `itk-daemon` | Coordinator daemon template (IPC server, shared memory writer) |
| `itk-overlay` | Transparent overlay window template (egui + wgpu, click-through) |
| `itk-native-dll` | Windows DLL injection template |
| `itk-ld-preload` | Linux LD_PRELOAD injection template |

### Tools

| Tool | Purpose |
|------|---------|
| `mem-scanner` | Memory pattern scanning utility for reverse engineering game processes |

### Project Layout

```
game-mods/
├── core/                           Shared ITK libraries
│   ├── itk-protocol/                 Wire protocol definitions
│   ├── itk-shmem/                    Cross-platform shared memory (seqlock)
│   ├── itk-ipc/                      Cross-platform IPC channels
│   ├── itk-sync/                     Clock synchronization
│   ├── itk-video/                    Video decode + frame management
│   └── itk-net/                      P2P networking (laminar)
├── daemon/                         Framework: coordinator daemon template
├── overlay/                        Framework: transparent overlay template
├── injectors/                      Framework: injection templates
│   ├── windows/native-dll/           Windows DLL injection
│   └── linux/ld-preload/             Linux LD_PRELOAD injection
├── projects/                       Game-specific mods
│   └── nms-cockpit-video/
│       ├── daemon/                   Video playback + audio daemon
│       ├── injector/                 Vulkan DLL injection (nightly, cdylib)
│       ├── overlay/                  Desktop overlay (egui + wgpu)
│       └── launcher/                Process orchestrator
├── tools/
│   └── mem-scanner/                Memory scanning utility
├── site/                           GitHub Pages documentation site
├── docs/                           Technical documentation
│   ├── ARCHITECTURE.md               Full architecture design doc
│   └── MIGRATION.md                  FlatBuffers migration plan
├── docker/                         CI Dockerfiles
└── .github/                        GitHub Actions workflows
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (Edition 2024, 13 workspace crates) |
| Serialization | bitcode (protocol), bincode (legacy via laminar) |
| Video | ffmpeg-next 7.0 (D3D11VA hw accel), cpal 0.15 (audio) |
| Graphics | ash 0.38 (Vulkan), wgpu 0.20, egui 0.28 |
| Hooking | retour 0.3 (function detours, static_detour) |
| Networking | laminar 0.5 (P2P UDP), tokio (async runtime) |
| Platform | windows 0.58 (Win32), nix 0.29 (Unix) |
| CI/CD | GitHub Actions (self-hosted runners, Docker containers) |

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/ARCHITECTURE.md) | Full technical design -- protocol, seqlock, security, performance budgets |
| [Migration Plan](docs/MIGRATION.md) | FlatBuffers migration roadmap (bincode -> cross-language) |
| [NMS Cockpit Video](projects/nms-cockpit-video/README.md) | Project docs -- building, usage, injector architecture, hook points |
| [CLAUDE.md](CLAUDE.md) | Claude Code agent instructions |
| [AGENTS.md](AGENTS.md) | Universal AI agent guidelines |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution policy (no external contributions) |
| [GitHub Pages Site](https://AndrewAltimit.github.io/game-mods/) | Interactive documentation and project showcase |

## Companion Repository

| Repository | Description |
|------------|-------------|
| [template-repo](https://github.com/AndrewAltimit/template-repo) | Parent project -- agent orchestration, MCP servers, CI/CD templates, security tooling |

## License

Dual-licensed under [Unlicense](LICENSE) and [MIT](LICENSE-MIT).
