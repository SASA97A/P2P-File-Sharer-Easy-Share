# ⚡ Easy Share (v2.0)

A blazingly fast, lightweight, and secure **local P2P file sharing desktop application** built with **Tauri 2.0** and **Rust**. Seamlessly transfer files and folders between devices on the same local network at full line speed — with zero internet connection or cloud servers required.

---

## 🚀 Key Features

- ⚡ **High-Performance Rust Core**: Powered by Tokio, Axum, and Reqwest with minimal memory footprint and native throughput.
- 📡 **Dual-Engine Auto Discovery**: Zero-configuration discovery using hybrid **mDNS** (`_easyshare._tcp.local`) and **UDP broadcast** (port 41234).
- 🧠 **Smart Physical NIC Filtering**: Automatically detects and scores active physical Wi-Fi/Ethernet adapters while filtering out virtual switches (Docker, WSL, Hyper-V, Tailscale, VPNs).
- 🔄 **Resumable Chunk Streaming**: 1 MB chunk streaming with seekable byte offsets. Interrupted transfers automatically resume from where they left off.
- 🛡️ **BLAKE3 Cryptographic Integrity**: Fast streaming BLAKE3 hashing verifies every file before committing to the download directory.
- 🔐 **Interactive Consent Handshake**: Full user control — receivers view sender device name, operating system, and file list before accepting or declining incoming transfers.
- 🏢 **Rooms / Group Isolation**: Set an optional Room ID to isolate peer discovery in busy office, school, or home networks.
- 🧭 **Manual IP & Port Connect**: Connect directly to remote peers across subnets or custom port configurations.
- 📁 **Atomic Storage & Safe Naming**: Safe path sanitization prevents directory traversal; files write to `.part` buffers and rename atomically with automatic collision resolution (`file(1).ext`).
- 🎨 **Modern Dark UI**: Clean, responsive interface featuring real-time transfer speed, progress bars, interactive modals, and peer status indicators.

---

## 🧩 Architecture & Tech Stack

- **Desktop Framework**: [Tauri 2.0](https://tauri.app/)
- **Backend Language**: [Rust](https://www.rust-lang.org/) (2021 edition)
  - **Asynchronous Runtime**: `tokio` (v1)
  - **HTTP Server**: `axum` (v0.8)
  - **HTTP Client**: `reqwest` (v0.12)
  - **Hashing**: `blake3`
  - **Service Discovery**: `mdns-sd` & `socket2` UDP broadcaster
  - **Serialization**: `serde` / `serde_json`
- **Frontend**: Vanilla JavaScript (ES Modules), HTML5, CSS3 Glassmorphism UI

---

## 📦 Prerequisites

Ensure you have the following installed:

1. **Bun**: (v1.0 or newer recommended) — [Install Bun](https://bun.sh/) *(or Node.js v18+)*
2. **Rust & Cargo**: (stable channel) — [Install Rust](https://www.rust-lang.org/tools/install)
3. **Platform Dependencies**:
   - **Windows**: Microsoft Visual Studio C++ Build Tools & WebView2 (pre-installed on Windows 10/11).
   - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
   - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

---

## 🛠️ Getting Started

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

## 🧪 Running Tests

Run the full Rust automated test suite, including storage, server, client, models, discovery, and end-to-end integration tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

To run individual test suites:
```bash
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

## 🔒 Security & Privacy

- **100% Local**: All transfers occur strictly peer-to-peer over your local network. No metadata or file content is ever transmitted to external cloud servers.
- **Explicit Consent**: Transfers cannot begin without explicit acceptance by the receiving device.
- **Hash Verified**: Every completed transfer validates against its BLAKE3 cryptographic hash to ensure zero corruption or tampering during transit.
- **Path Sanitization**: Inbound file names are strictly sanitized to prevent path traversal attacks.

---

## 📄 License

This project is licensed under the ISC License.
