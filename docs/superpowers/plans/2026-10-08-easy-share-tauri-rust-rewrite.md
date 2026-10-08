# Easy Share (Rust + Tauri 2.0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild Easy Share as a high-performance, cross-platform P2P file sharing application using Rust and Tauri 2.0, featuring resumable transfers, explicit consent dialogs, mDNS/UDP discovery, and room-based grouping.

**Architecture:** A native Tauri 2.0 desktop application pairing a modern lightweight WebView frontend with an embedded asynchronous Rust engine. Sockets and transfers run on `Tokio` + `Axum` utilizing a standardized HTTP/1.1 chunking protocol with `.part` file persistence, BLAKE3 checksum verification, and NIC-filtered peer discovery.

**Tech Stack:** Rust 2021/2024, Tauri 2.x, Tokio, Axum, reqwest, mdns-sd, blake3, serde/serde_json, HTML5/CSS3/ES Modules.

**Spec:** `docs/superpowers/specs/2026-10-08-easy-share-tauri-rust-design.md`

---

## Global Constraints

- **Platform Priority:** Windows 10/11 first-class priority, structured for seamless Linux, macOS, Android, and iOS builds.
- **Port Strategy:** Default TCP port `5050`, UDP discovery port `41234` with `SO_REUSEADDR` / fallback port binding.
- **Security:** Strict path sanitization on all incoming filenames (`path::Path::file_name()` only; forbid separators and Windows reserved devices `CON`, `PRN`, `AUX`, `NUL`, `COM1-9`).
- **Memory Footprint:** Buffer chunk sizes strictly bounded (64 KB to 4 MB); zero full-file buffering in memory.
- **Data Integrity:** BLAKE3 SIMD hashing calculated on sender and verified on receiver before committing files.

---

## Review Focus

1. **Path Traversal Attacks:** Input filenames with `../` or absolute drive paths (`C:\`) must be stripped to safe local filenames in the Downloads folder.
2. **Network Interruption Mid-Stream:** Sockets disconnecting mid-file must preserve `.part` progress, report accurate byte offsets upon reconnection, and resume without corruption.
3. **Sender Spam / Consent Bypass:** Any chunk upload attempted without an active, user-accepted `session_token` must be rejected with `403 Forbidden`.
4. **Virtual NIC Traps:** Sockets must filter out Docker, WSL, VMware, and VPN adapters so peer discovery advertises reachable physical Wi-Fi / LAN IPs.
5. **Concurrent File Transfers:** Multiple files queued in a transfer request must transfer sequentially or in coordinated streams without file descriptor leaks or race conditions on `.part` files.

---

## Task Breakdown

### Task 1: Project Scaffolding & Tauri 2.0 Setup

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Modify: `package.json`

**Interfaces:**
- Produces: Tauri 2.0 project skeleton with Cargo workspace dependencies (`tokio`, `axum`, `reqwest`, `serde`, `serde_json`, `blake3`, `uuid`, `mdns-sd`, `local_ip_address`).

- [ ] **Step 1: Verify / Install Rust toolchain**
  Check `rustc --version` and `cargo --version`. If absent, run rustup installer or report installation command.
- [ ] **Step 2: Initialize Tauri 2.0 configuration and `src-tauri/Cargo.toml`**
  Configure `Cargo.toml` with `tauri`, `tokio` (features = `["full"]`), `axum`, `reqwest` (`["stream"]`), `serde`, `serde_json`, `blake3`, `uuid` (`["v4"]`), `mdns-sd`, `local_ip_address`.
- [ ] **Step 3: Configure `src-tauri/tauri.conf.json`**
  Set app identifier `com.easyshare.app`, window dimensions (820x600), title "Easy Share", and frontend dist directory.
- [ ] **Step 4: Create minimal `src-tauri/src/main.rs` and `lib.rs`**
  Verify compilation with `cargo check` inside `src-tauri`.
- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri package.json
  git commit -m "feat: scaffold Tauri 2.0 and Rust workspace dependencies"
  ```

---

### Task 2: Core Data Types & Protocol Models

**Files:**
- Create: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/models/peer.rs`
- Create: `src-tauri/src/models/transfer.rs`
- Test: `src-tauri/tests/models_test.rs`

**Interfaces:**
- Produces:
  - `PeerInfo`: `{ device_name: String, device_type: String, os: String, room_id: Option<String>, ip: String, port: u16 }`
  - `TransferRequest`: `{ request_id: String, sender_name: String, sender_os: String, room_id: Option<String>, total_bytes: u64, files: Vec<FileMetadata> }`
  - `FileMetadata`: `{ file_id: String, name: String, size: u64, blake3_hash: Option<String> }`
  - `TransferResponse`: `{ status: TransferStatus, session_token: Option<String>, existing_offsets: HashMap<String, u64>, reason: Option<String> }`
  - `ChunkHeader`: `{ session_token: String, file_id: String, offset: u64 }`

- [ ] **Step 1: Write failing unit test for serialization / deserialization**
  In `src-tauri/tests/models_test.rs`, test that `TransferRequest` and `PeerInfo` round-trip through JSON correctly.
- [ ] **Step 2: Run test to verify failure**
  Run: `cargo test --test models_test` (fails: module not found).
- [ ] **Step 3: Implement models in `src/models/peer.rs` and `src/models/transfer.rs`**
  Add `#[derive(Debug, Clone, Serialize, Deserialize)]` annotations and helper constructors.
- [ ] **Step 4: Run test to verify pass**
  Run: `cargo test --test models_test` (PASS).
- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/src/models src-tauri/tests/models_test.rs
  git commit -m "feat: add protocol models and serialization tests"
  ```

---

### Task 3: File I/O & Safe Storage Engine

**Files:**
- Create: `src-tauri/src/storage/mod.rs`
- Create: `src-tauri/src/storage/file_manager.rs`
- Test: `src-tauri/tests/storage_test.rs`

**Interfaces:**
- Produces:
  - `FileManager::sanitize_filename(name: &str) -> String`
  - `FileManager::get_downloads_dir() -> PathBuf`
  - `FileManager::get_part_path(downloads_dir: &Path, file_name: &str, session_token: &str) -> PathBuf`
  - `FileManager::get_unique_dest_path(downloads_dir: &Path, file_name: &str) -> PathBuf`
  - `FileManager::commit_part_file(part_path: &Path, dest_path: &Path) -> Result<(), StorageError>`

- [ ] **Step 1: Write unit tests for path traversal attacks and unique naming**
  In `src-tauri/tests/storage_test.rs`, test malicious names (`../../test.exe`, `C:\Windows\System32\cmd.exe`, `CON.txt`, `file/with/subdirs.zip`).
- [ ] **Step 2: Run test to verify failure**
  Run: `cargo test --test storage_test`.
- [ ] **Step 3: Implement `FileManager`**
  Implement strict filename sanitization, `.part` path derivation, duplicate naming collision resolution (`file (1).ext`), and atomic rename.
- [ ] **Step 4: Run test to verify pass**
  Run: `cargo test --test storage_test` (PASS).
- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/src/storage src-tauri/tests/storage_test.rs
  git commit -m "feat: implement safe storage manager with path traversal sanitization"
  ```

---

### Task 4: Embedded HTTP Transfer Server (Axum)

**Files:**
- Create: `src-tauri/src/server/mod.rs`
- Create: `src-tauri/src/server/routes.rs`
- Create: `src-tauri/src/server/state.rs`
- Test: `src-tauri/tests/server_test.rs`

**Interfaces:**
- Produces:
  - `ServerState`: Manages pending requests, active sessions, and consent channels.
  - `start_server(state: Arc<ServerState>, port: u16) -> Result<(u16, JoinHandle<()>), ServerError>`
  - Route handlers for `GET /api/v1/info`, `POST /api/v1/transfer/request`, `GET /api/v1/transfer/status`, `POST /api/v1/transfer/chunk`, `POST /api/v1/transfer/finish`.

- [ ] **Step 1: Write integration tests for server endpoints**
  In `src-tauri/tests/server_test.rs`, test:
  1. `GET /api/v1/info` returns 200 with device info.
  2. `POST /api/v1/transfer/request` triggers consent and returns session token.
  3. `POST /api/v1/transfer/chunk` streams binary bytes directly to `.part` file at given offset.
  4. `POST /api/v1/transfer/finish` validates size/hash and commits file.
- [ ] **Step 2: Run test to verify failure**
  Run: `cargo test --test server_test`.
- [ ] **Step 3: Implement `ServerState` and Axum route handlers**
  Stream request bodies with `axum::body::Body` and `tokio::io::AsyncWriteExt` with seeking for offset support.
- [ ] **Step 4: Run test to verify pass**
  Run: `cargo test --test server_test` (PASS).
- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/src/server src-tauri/tests/server_test.rs
  git commit -m "feat: implement Axum embedded transfer server and chunk streamer"
  ```

---

### Task 5: HTTP Client & Resumable Transfer Pipeline

**Files:**
- Create: `src-tauri/src/client/mod.rs`
- Create: `src-tauri/src/client/sender.rs`
- Test: `src-tauri/tests/client_test.rs`

**Interfaces:**
- Produces:
  - `TransferClient::request_transfer(peer: &PeerInfo, request: &TransferRequest) -> Result<TransferResponse, ClientError>`
  - `TransferClient::send_file_resumable(peer: &PeerInfo, session_token: &str, file: &FileToSend, progress_cb: impl Fn(u64, u64)) -> Result<(), ClientError>`

- [ ] **Step 1: Write integration test for sender pipeline against embedded test server**
  In `src-tauri/tests/client_test.rs`, simulate sending a 10 MB payload, interrupting it at 5 MB, and resuming to complete transfer with matching BLAKE3 hash.
- [ ] **Step 2: Run test to verify failure**
  Run: `cargo test --test client_test`.
- [ ] **Step 3: Implement `TransferClient`**
  Implement chunked reading (`tokio::fs::File`), seeking to resumed offsets, streaming chunks with `reqwest`, and reporting progress to callbacks.
- [ ] **Step 4: Run test to verify pass**
  Run: `cargo test --test client_test` (PASS).
- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/src/client src-tauri/tests/client_test.rs
  git commit -m "feat: implement resumable client transfer engine with offset recovery"
  ```

---

### Task 6: Network Discovery Engine (mDNS & UDP Broadcast)

**Files:**
- Create: `src-tauri/src/discovery/mod.rs`
- Create: `src-tauri/src/discovery/nic.rs`
- Create: `src-tauri/src/discovery/mdns.rs`
- Create: `src-tauri/src/discovery/udp.rs`
- Test: `src-tauri/tests/discovery_test.rs`

**Interfaces:**
- Produces:
  - `get_best_physical_ip() -> Option<IpAddr>`
  - `DiscoveryService::start(device_info: DeviceInfo, on_peer: impl Fn(PeerEvent))`

- [ ] **Step 1: Write unit tests for physical NIC detection and discovery packet parsing**
  In `src-tauri/tests/discovery_test.rs`, test filtering out virtual network names and parsing mDNS/UDP heartbeat payloads.
- [ ] **Step 2: Run test to verify failure**
  Run: `cargo test --test discovery_test`.
- [ ] **Step 3: Implement `nic.rs`, `mdns.rs`, and `udp.rs`**
  Use `mdns-sd` for zeroconf discovery and `tokio::net::UdpSocket` for UDP broadcast with SO_REUSEADDR and 5-second interval.
- [ ] **Step 4: Run test to verify pass**
  Run: `cargo test --test discovery_test` (PASS).
- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/src/discovery src-tauri/tests/discovery_test.rs
  git commit -m "feat: implement dual mDNS and UDP peer discovery with NIC filtering"
  ```

---

### Task 7: Tauri State Management & IPC Bridge

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/commands.rs`

**Interfaces:**
- Produces Tauri Commands:
  - `get_my_device_info()`
  - `select_files()`
  - `start_transfer(peer_id, files)`
  - `respond_transfer_request(request_id, accept: bool)`
  - `set_room_id(room: Option<String>)`
  - `add_manual_peer(ip, port)`
- Produces Tauri Events:
  - `peer-found`, `peer-lost`, `transfer-requested`, `transfer-progress`, `transfer-completed`, `transfer-error`

- [ ] **Step 1: Write Tauri command handlers in `src-tauri/src/commands.rs`**
  Bridge Rust backend state (`Arc<AppState>`) to frontend IPC.
- [ ] **Step 2: Register commands and start background services in `src-tauri/src/lib.rs`**
  Initialize discovery, HTTP server, and state singletons on app setup.
- [ ] **Step 3: Verify Tauri build and command binding**
  Run: `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] **Step 4: Commit**
  ```bash
  git add src-tauri/src/commands.rs src-tauri/src/lib.rs
  git commit -m "feat: wire Tauri IPC commands and real-time frontend event emitters"
  ```

---

### Task 8: Frontend UI & Consent Modal

**Files:**
- Modify: `src/index.html`
- Modify: `src/styles/styles.css`
- Create: `src/script/tauri-bridge.js`
- Modify: `src/script/renderer.js`
- Modify: `src/script/ui.js`
- Modify: `src/script/transfer.js`
- Modify: `src/script/peers.js`
- Modify: `src/script/files.js`

**Interfaces:**
- Produces:
  - Interactive Consent Modal: Shows Sender, File count, Size, and Accept / Decline buttons.
  - Device Radar with Room Badges & OS Icons (`🖥️ Windows/Mac/Linux`, `📱 Mobile`).
  - Active Progress Dashboard with live speed (MB/s) and ETA.
  - Restored, fully functional Drag-and-Drop zone.

- [ ] **Step 1: Update `src/index.html` with Consent Dialog markup and Room controls**
  Add modal overlay `#consent-modal` and room tag selector.
- [ ] **Step 2: Update `src/styles/styles.css`**
  Add modal styling, fix `.device` relative positioning for checkmarks, and add responsive scroll areas.
- [ ] **Step 3: Implement `src/script/tauri-bridge.js`**
  Provide clean wrapper over `@tauri-apps/api` invoke & listen calls.
- [ ] **Step 4: Update `ui.js`, `peers.js`, `files.js`, and `transfer.js`**
  Hook UI events into Tauri IPC with persistent device selection and live progress tracking.
- [ ] **Step 5: Commit**
  ```bash
  git add src/
  git commit -m "feat: implement modern UI with consent modal, room tags, and drag-and-drop"
  ```

---

### Task 9: End-to-End Integration & Verification

**Files:**
- Test: `src-tauri/tests/e2e_transfer_test.rs`
- Modify: `README.md`

- [ ] **Step 1: Write automated end-to-end integration test**
  Spawns two local node instances (Node A on port 5050, Node B on port 5051). Node A sends 50 MB file to Node B $\rightarrow$ Node B auto-accepts $\rightarrow$ verify exact file size, BLAKE3 checksum, and `.part` file cleanup.
- [ ] **Step 2: Run test suite to verify full pass**
  Run: `cargo test --all`
  Expected: ALL TESTS PASS.
- [ ] **Step 3: Update `README.md` with build & run instructions for Tauri 2.0**
- [ ] **Step 4: Commit**
  ```bash
  git add src-tauri/tests README.md
  git commit -m "test: add full end-to-end transfer integration test and update README"
  ```
