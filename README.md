# Monitor — Container & Host Monitoring System

A cross-platform desktop/terminal/web monitoring tool built with **Tauri v2**. It shows live processes, CPU/RAM/temperatures, GPU adapters & usage, Docker/Podman containers, libvirt VMs, Kubernetes pods, open ports and an interactive network map — all in one native app.

One Rust backend, three ways to run it:

| Mode | Command | What you get |
|---|---|---|
| **GUI** (default) | `monitor` | Native desktop window (TauriWebView) |
| **CLI TUI** | `monitor --cli` | btop-style terminal UI (ratatui/crossterm) |
| **Headless server** | `monitor --server [--bind=X] [--port=N] [--token=T]` | Axum HTTP + WebSocket server on port 7333 (default), serve the same dashboard from any browser |

CLI also accepts `--port=X` / `--bind=X` in CLI/TUI mode, and `--help` / `-h` prints all flags.

---

## Features

- **Host page** — CPU, memory, temperatures (sysinfo components), GPU adapters/usage
  (Linux native, Windows PDH + `nvidia-smi`, macOS `system_profiler`)
- **Processes** — full process list with sorting, kill support
- **Runtimes** — Docker & Podman containers (bollard), libvirt/KubeVirt VMs, Kubernetes pods & events
- **Network map** — interactive graph (cytoscape) of connections via a shared socket layer
  (Linux native, macOS `lsof`/`nettop`, Windows `netstat`)
- **Ports & sockets** — per-process socket discovery (`procnet.rs`, `ports.rs`)
- **Logs** — streaming container/process logs
- **Web UI in server mode** — same SolidJS dashboard, no desktop needed

---

## Architecture

```
┌──────────────────────────────────────────────┐
│  Frontend (src/)                             │
│  SolidJS + Tailwind + uPlot + cytoscape      │
│  views/: Overview, Host, runtime/, k8s/,     │
│          network/, VmDashboard               │
└──────────────┬───────────────────────────────┘
               │ Tauri IPC (commands/events)  or  WebSocket (server mode)
┌──────────────┴───────────────────────────────┐
│  Rust backend (src-tauri/src/)               │
│  host.rs  temp.rs  gpu/  procnet.rs  ports.rs│
│  netmap/  topology/  ping.rs                 │
│  ─────────────────────────────────────────── │
│  engine/   Runtime detection & state         │
│    ├ docker.rs      (Docker/Podman, bollard) │
│    ├ kubernetes.rs  (kube client)            │
│    ├ vm.rs          (libvirt/KubeVirt)       │
│    ├ logs.rs        (log streaming)          │
│    └ detect.rs      (runtime discovery)      │
│  ─────────────────────────────────────────── │
│  server/     axum HTTP + WS (--server mode)  │
│  cli/        ratatui TUI        (--cli mode) │
└──────────────────────────────────────────────┘
```

The backend keeps shared caches in a `Backend` struct (`src-tauri/src/lib.rs`). Pollers refresh
host stats, runtimes, network map and logs on timers; consumers are:
the Tauri event loop (GUI), the CLI TUI, and the axum WebSocket server.

In server mode the same compiled web asset bundle is served over HTTP, so a browser
anywhere on the network (machines with `--bind 0.0.0.0`) can view the dashboard.
An optional `--token` protects the WebSocket endpoints.

---

## Prerequisites

### All platforms
- **Node.js 22+** and **pnpm 10** (`corepack enable` is the easiest way)
- **Rust stable** via [rustup](https://rustup.rs)
- pnpm needs to allow build scripts on first install (see Troubleshooting)

### Linux (Debian/Ubuntu)
```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf
```
Additional runtime detection (optional):
- Docker: user in the `docker` group, or rootless podman socket
- libvirt VMs: `libvirtd` running

### Windows
- [Rust](https://rustup.rs) with the MSVC toolchain
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (preinstalled on Win 11)
- [NSIS](https://nsis.io/) or WiX only if you want custom installer tweaks (tauri handles bundling)
- `nvidia-smi` on PATH for NVIDIA GPU stats (optional)

### macOS
- Xcode Command Line Tools (`xcode-select --install`)

---

## Running from source

```bash
# 1. install frontend deps
pnpm install

# 2. run the desktop app in dev mode (hot reload)
pnpm tauri dev

# 3. production build / installer bundles
pnpm tauri build
# output: src-tauri/target/release/bundle/...
```

### Headless server mode

```bash
# plain build (no bundling needed to use server mode)
cargo build --release --manifest-path src-tauri/Cargo.toml

# run it
src-tauri/target/release/monitor --server                 # 0.0.0.0:7333, no auth
src-tauri/target/release/monitor --server --port=9000 \
    --bind=0.0.0.0 --token=s3cret                        # protected WS
# open http://<host>:7333 in a browser
```

### CLI TUI mode

```bash
src-tauri/target/release/monitor --cli
```

---

## CI & Releases

- `.github/workflows/ci.yml` — on every push/PR: type-checks + builds the frontend, and runs
  `cargo check` on Linux, Windows and (optionally) macOS runners.
- `.github/workflows/release.yml` — on a `v*` tag or manual dispatch:
  builds **Linux** (`.deb`, `.AppImage`) and **Windows** (`.msi`, NSIS `.exe`) installers with
  `tauri-apps/tauri-action`, attaches them to a GitHub Release, and uploads the artifacts
  to a matching **Codeberg release** via the Forgejo API.

To publish a release:

```bash
git tag v0.1.0
git push origin v0.1.0
git push gh    v0.1.0
```

Required GitHub secrets for Codeberg upload:
- `CODEBERG_TOKEN` — a Codeberg access token (Settings → Applications → Access tokens, scope `repository` / `write:issue` not needed; write access to repo releases is enough)

---

## Project layout

```
src/                  Frontend (SolidJS)
  views/              Screens: Overview, Host, runtime/, k8s/, network/
  components/         Shared UI components
  hooks/              useBackend (data transport), useLogStream
  lib/                format.ts, ports.ts, services.ts, transport.ts
  stores/ types/      Solid stores & shared TypeScript types
src-tauri/
  src/host.rs         Poller for CPU/RAM/temp/host info
  src/gpu/            Per-OS GPU discovery
  src/engine/         Docker/Podman/k8s/VM engine
  src/netmap/         Network topology/map
  src/server/         Axum WebSocket server
  src/cli/            Terminal UI
  tauri.conf.json     Tauri config (window, bundle targets, icons)
```

---

## Troubleshooting

- **pnpm blocks build scripts** — when prompted, allow the build scripts of the
  listed packages (esbuild etc.), or run `pnpm config set enable-pre-post-scripts true`
  and reinstall.
- **`cargo check` missing webkit on Linux** — install the apt packages listed above.
- **No containers shown** — check that the Docker/Podman socket is reachable
  (`docker ps` / `podman ps` work as your user).
- **Server mode port busy** — change with `--port=`.
- **Windows GPU stats empty** — ensure `nvidia-smi` is on PATH for NVIDIA cards; Intel/AMD
  fall back to PDH counters.

---

## License

TBD — add your license here (MIT/Apache-2.0/GPL recommended for Tauri projects).
