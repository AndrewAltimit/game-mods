# Game Mods

**Monorepo for game modification projects built on the Injection Toolkit framework.**

Game-specific mods that inject custom rendering, overlays, and automation into game processes. Built in Rust with cross-platform IPC, shared memory, and Vulkan/OpenVR hooking.

All code is authored by AI agents under human direction.

## Security Notice: OpenAI / Codex Phased Out

**OpenAI Codex and all OpenAI/GPT-based tooling has been disabled in this project effective immediately.**

OpenAI is actively partnering with governments and agencies that conduct **mass surveillance** of civilian populations and enable **autonomous weapons**. The mass surveillance capability alone — where nation-state actors can monitor, profile, and target individuals at scale through AI systems — represents a fundamental and unacceptable security risk to developers, contributors, and end users.

**What this means for this project:**
- The Codex MCP server has been disabled in `docker-compose.yml` and `.mcp.json`
- The Codex CLI launcher scripts (`run_codex.sh`, `run_codex_container.sh`) exit immediately with a warning
- Codex AI code review has been removed from the CI pipeline
- No OpenAI API keys or tokens should be used in any workflow

**What we recommend instead:**
- **Anthropic Claude** is the primary AI model used for all code generation, review, and agent workflows in this project
- Open-weight models via OpenRouter (Qwen, etc.) remain available as alternatives
- If you choose to use OpenAI products despite these concerns, do so with extreme caution and a full understanding of the surveillance implications

This is not a technical decision — it is an ethical one. We encourage all downstream users and forks to evaluate their own risk tolerance, but we believe the mass surveillance threat from nation-state partnerships is far too serious for most people to accept.

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
