# Easy Share (v2.0)

A high-performance, lightweight, and secure **local P2P file sharing desktop application** built with **Tauri 2.0** and **Rust**. Transfer files and folders between devices on the same local network at full line speed with zero internet connectivity or cloud infrastructure required.

---

## Key Features

- **High-Performance Rust Core**: Powered by Tokio, Axum, and Reqwest with minimal memory footprint and native throughput.
- **6-Digit Numeric PIN & QR Code Pairing**: Pair devices in seconds using a 6-digit PIN (`XXX - XXX`) or visual SVG QR code generated entirely on-device.
- **Enterprise Network & AP Isolation Bypass**: Designed for restrictive corporate, university, and guest Wi-Fi networks where multicast/broadcast or client isolation blocks standard discovery.
- **Fast /24 Subnet Sweeping**: Asynchronous, high-concurrency subnet scanning with Tokio workers and 200ms per-host timeouts to pair peers without multicast.
- **Direct IP, Port & URI Connect**: Unicast manual connect supporting IP addresses, `IP:Port`, HTTP URLs, and `easyshare://pair?...` protocol URLs to bridge across subnets or routed networks.
- **Dual-Engine Auto Discovery**: Zero-configuration discovery using hybrid **mDNS** (`_easyshare._tcp.local`) and **UDP broadcast** (port 41234) for unrestricted LAN environments.
- **Smart Physical NIC Filtering**: Automatically detects and scores active physical Wi-Fi and Ethernet adapters while filtering out virtual interfaces (Docker, WSL, Hyper-V, Tailscale, VPNs).
- **Resumable Chunk Streaming**: 1 MB chunk streaming with seekable byte offsets. Interrupted transfers automatically resume from where they left off.
- **BLAKE3 Cryptographic Integrity**: Fast streaming BLAKE3 hashing verifies every file before committing to the download directory.
- **Interactive Consent Handshake**: Receivers inspect sender device name, operating system, and file list before accepting or declining incoming transfers.
- **Rooms / Group Isolation**: Optional Room ID to partition peer visibility on busy networks.
- **Atomic Storage & Safe Naming**: Safe path sanitization prevents directory traversal attacks; incoming files write to `.part` buffers and rename atomically with collision handling.
- **Modern Responsive Dark UI**: Clean interface built with glassmorphism aesthetics, Lucide SVG icons, real-time transfer telemetry, and paired peer badges.

---

## Corporate & Enterprise Network Support

In many enterprise, university, and public Wi-Fi networks, conventional peer discovery fails due to:
1. **AP Client Isolation**: Access points restrict wireless clients from sending broadcast or multicast traffic to neighboring clients.
2. **mDNS & Multicast Suppression**: Network switches drop mDNS (`224.0.0.251`) and UDP broadcast packets to minimize network chatter.
3. **VLAN Segmentation**: Devices reside on different subnets or sub-interfaces where broadcast packets do not traverse.

### How Easy Share Solves This

Easy Share provides two robust offline mechanisms to establish direct unicast peer connections in restricted environments:

1. **6-Digit PIN Subnet Sweeper**:
   - When you enter a 6-digit PIN provided by another device, Easy Share determines your physical network adapter's local `/24` subnet.
   - It fires concurrent, lightweight HTTP probe requests across the subnet with a strict 200ms timeout per host.
   - The device matching the PIN responds, handshake completes, and the peer is immediately added to your paired devices list.

2. **Direct QR Code / Unicast Connect**:
   - Scan or copy the `easyshare://pair?ip=...&port=...&pin=...` URI or enter the destination `IP:Port` directly.
   - Easy Share reaches across routed subnets via direct unicast HTTP, bypassing broadcast filters entirely.

All pairing mechanisms are **100% offline and pure-LAN**: no external signaling servers, STUN/TURN servers, or internet connections are ever contacted.

---

## Architecture & Tech Stack

- **Desktop Framework**: [Tauri 2.0](https://tauri.app/)
- **Backend Language**: [Rust](https://www.rust-lang.org/) (2021 edition)
  - **Asynchronous Runtime**: `tokio` (v1)
  - **HTTP Server**: `axum` (v0.8)
  - **HTTP Client**: `reqwest` (v0.12)
  - **QR Synthesis**: `qrcode`
  - **Hashing**: `blake3`
  - **Service Discovery**: `mdns-sd` & `socket2` UDP broadcaster
  - **Serialization**: `serde` / `serde_json`
- **Frontend**: Vanilla JavaScript (ES Modules), HTML5, CSS3 Glassmorphism UI, [Lucide](https://lucide.dev/) Icons

---

## Prerequisites

Ensure you have the following installed:

1. **Bun** (v1.0 or newer recommended) — [Install Bun](https://bun.sh/) *(or Node.js v18+)*
2. **Rust & Cargo** (stable channel) — [Install Rust](https://www.rust-lang.org/tools/install)
3. **Platform Dependencies**:
   - **Windows**: Microsoft Visual Studio C++ Build Tools & WebView2 (pre-installed on Windows 10/11).
   - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
   - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

---

## Getting Started

### 1. Clone the repository
```bash
git clone https://github.com/SASA97A/P2P-File-Sharer-Easy-Share.git
cd P2P-File-Sharer-Easy-Share
```

### 2. Install frontend dependencies
```bash
bun install
```

### 3. Run in Development Mode
Launches the native Tauri desktop window:
```bash
bun run dev
# or
bun run tauri dev
```

### 4. Build Production Application
Compiles the optimized Rust backend and packages the native executable/installer:
```bash
bun run build
# or
bun run tauri build
```
The compiled binaries and platform bundles (`.msi`, `.dmg`, `.deb`, `.AppImage`) will be available in `src-tauri/target/release/bundle/`.

---

## Running Tests

Run the full Rust automated test suite, covering pairing, subnet scanning, storage, server, client, models, discovery, and end-to-end transfers:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

To run individual test suites:
```bash
# Pairing & PIN / QR Code Tests
cargo test --manifest-path src-tauri/Cargo.toml --test pairing_test

# Subnet Sweeper & Address Resolution Tests
cargo test --manifest-path src-tauri/Cargo.toml --test scanner_test

# Tauri AppState & IPC Commands Tests
cargo test --manifest-path src-tauri/Cargo.toml --test app_state_and_commands_test

# End-to-End Transfer & Resumption Tests
cargo test --manifest-path src-tauri/Cargo.toml --test e2e_transfer_test

# Storage & File Manager Tests
cargo test --manifest-path src-tauri/Cargo.toml --test storage_test

# Client Streaming & Handshake Tests
cargo test --manifest-path src-tauri/Cargo.toml --test client_test

# Server Endpoint Tests
cargo test --manifest-path src-tauri/Cargo.toml --test server_test

# Discovery & NIC Selection Tests
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_test
```

---

## Security & Privacy

- **100% Local & Offline**: All transfers, PIN verifications, and discovery processes operate strictly peer-to-peer over your local network. No metadata or file content is ever transmitted to external cloud servers.
- **Explicit Consent**: Transfers cannot begin without explicit user acceptance on the receiving device.
- **Hash Verified**: Every completed transfer validates against its BLAKE3 cryptographic hash to ensure zero corruption or tampering during transit.
- **Path Sanitization**: Inbound file names are strictly sanitized to prevent directory traversal attacks.
- **Regenerable PINs**: Pairing PINs can be regenerated at any time with a single click to invalidate prior pairing sessions.

---

## License

This project is licensed under the ISC License.
