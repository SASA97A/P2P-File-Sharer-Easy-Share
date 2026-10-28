# Easy Share - Shipped Features & Roadmap

This document catalogs the implemented architectural enhancements and upcoming roadmap candidates for Easy Share following the Tauri 2.0 Rust rewrite.

---

# Part 1: Completed & Shipped Features

The following features have been fully implemented, integrated into the UI and backend, and verified with automated test suites.

---

## 1. Directory Tree Preservation for Folder Transfers

### Overview
Users can stage and transfer entire directory hierarchies. The receiver reconstructs the complete directory structure in their designated download directory rather than flattening files.

### Delivered Capabilities
- **Relative Path Manifest**: Inspects and transmits relative subpaths alongside file payloads in `TransferRequestPayload` (e.g., `project/src/main.rs`).
- **Receiver Folder Construction**: Automatically sanitizes and creates intermediate directories in the receiver's target download folder before streaming chunks.
- **Visual Staging Hierarchy**: Preserves relative paths in the UI file staging list with folder badges and formatted file metrics.
- **Collision Avoidance**: Safely resolves path collisions and avoids path traversal attacks via strict path normalization.

### Technical Implementation
- **Rust Backend**: `inspect_file_paths` in `src-tauri/src/commands.rs` recursively walks folder paths and emits relative paths; `src-tauri/src/server/routes.rs` handles folder creation using `tokio::fs::create_dir_all`.
- **Frontend**: Staged items in `src/script/files.js` and `src/script/transfer.js` preserve `relative_path` attributes for payloads.
- **Tests**: `src-tauri/tests/app_state_and_commands_test.rs` (`test_inspect_file_paths_preserves_relative_directory_tree`).

---

## 2. End-to-End Encryption (E2EE) with PIN-Based Handshake

### Overview
Local transfers can be authenticated and encrypted end-to-end to protect sensitive files from eavesdropping on untrusted, shared, or public Wi-Fi networks.

### Delivered Capabilities
- **Authenticated Key Exchange**: Derives ephemeral symmetric session keys from the 6-digit pairing PIN using HKDF key derivation.
- **Authenticated Encryption**: Encrypts in-flight streaming chunks using **ChaCha20-Poly1305** AEAD authenticated encryption.
- **Tamper Verification**: Cryptographic tags on every chunk guarantee tamper-proof delivery without external certificate authorities.
- **User Settings & Toggle**: Configurable E2EE switch in the Settings drawer with persistent state across app restarts.

### Technical Implementation
- **Rust Backend**: Cryptographic primitives in `src-tauri/src/crypto.rs`; integrated into `TransferClient`, `sender.rs`, and server chunk streaming routes (`routes.rs`); commands `get_encryption_settings` and `set_encryption_settings` in `commands.rs`.
- **Frontend**: Settings toggle in `src/script/settings.js` with persistence via Tauri IPC bridge.
- **Tests**: `src-tauri/tests/client_test.rs` and `src-tauri/tests/app_state_and_commands_test.rs`.

---

## 3. Bandwidth Throttling / Speed Limiter

### Overview
Allows users to configure transfer speed limits to prevent saturating the local network or router bandwidth during large transfers.

### Delivered Capabilities
- **Global & Live Presets**: Selectable speed presets (Unlimited, 5 MB/s, 10 MB/s, 25 MB/s, 50 MB/s) or custom bandwidth limits.
- **Live On-the-Fly Adjustment**: Changes to speed limits take effect immediately during active transfers without pausing or interrupting the transfer session.
- **Persistent Defaults**: Configurable default speed cap saved in app settings.

### Technical Implementation
- **Rust Backend**: Token bucket rate-limiter in `src-tauri/src/client/sender.rs` using atomic bandwidth tracking (`AtomicU64`); IPC commands `get_speed_limit` and `set_speed_limit` in `src-tauri/src/commands.rs`.
- **Frontend**: Bandwidth controls in Settings drawer and dynamic controls in `src/script/settings.js` and `src/script/transfer.js`.
- **Tests**: `src-tauri/tests/client_test.rs` and `src-tauri/tests/settings_test.rs`.

---

## 4. System Tray & Native OS Desktop Notifications

### Overview
Allows Easy Share to run smoothly in the background, minimizing to the system tray and delivering OS desktop notifications for transfer requests and completion events.

### Delivered Capabilities
- **System Tray Integration**: Native tray icon with contextual menu options: "Open Easy Share", "Pause All Transfers", "Clear Finished", and "Quit".
- **Minimize to Tray**: Optional setting to minimize to the notification area instead of closing the application.
- **Native OS Notifications**: Desktop notifications for incoming transfer consent requests, transfer completion batches, and connection errors.
- **Actionable Notification Handling**: Notifications indicate incoming sender device name, file count, and payload size.

### Technical Implementation
- **Rust Backend**: Tauri tray setup in `src-tauri/src/tray.rs` and notifications via `tauri-plugin-notification`; settings persisted in `AppState`.
- **Frontend**: System tray and notification toggle switches in the Settings view (`src/script/settings.js`).
- **Tests**: `src-tauri/tests/app_state_and_commands_test.rs`.

---

## 5. Native Tauri 2 Drag-and-Drop Staging with Directory Hierarchy

### Overview
Delivers native operating system drag-and-drop file and directory staging into the Easy Share window, bypassing browser webview file sandbox limitations and preserving full relative directory hierarchies.

### Delivered Capabilities
- **Tauri 2 Window Drop Integration**: Captures native window file drops using `getCurrentWebviewWindow().onDragDropEvent` across Windows, macOS, and Linux.
- **Deep Recursive Directory Inspection**: Recursively walks folders dropped into the window, extracting metadata (relative paths, sizes, item counts) in background threads.
- **Staging Area Visual Feedback**: Active drag hover styling and dynamic file/folder counter staging cards.
- **Dual Fallbacks**: Robust fallback chain supporting `getCurrentWebviewWindow().onDragDropEvent`, legacy window event bridges, mock event bus for testing, and DOM DataTransfer fallback.

### Technical Implementation
- **Rust Backend**: `inspect_file_paths` command in `src-tauri/src/commands.rs` recursively walks folder paths and preserves relative folder hierarchy.
- **Frontend**: Tauri drag-drop handler in `src/script/tauri-bridge.js` and window event routing in `src/script/renderer.js` and `src/script/files.js`.
- **Tests**: End-to-end unit test suite in `tests/frontend_drag_drop.test.js`.

---

## 6. Multi-Interface Subnet Scanning, Ethernet Prioritization & Device Deduplication

### Overview
Enables seamless multi-network peer discovery and PIN pairing across both Wi-Fi and Ethernet interfaces, prioritizing high-speed wired Ethernet connections for file transfers while avoiding duplicate peer cards in the UI.

### Delivered Capabilities
- **Physical Ethernet Prioritization**: Interface scoring awards physical Ethernet adapters (+50) higher priority over Wi-Fi (+20), ensuring high-speed LAN links are used whenever available.
- **Deduplicated Multi-Subnet Sweeps**: Discovers and scans unique `/24` subnets across all active host interfaces (Ethernet first, then Wi-Fi) with immediate short-circuiting upon PIN match.
- **Cross-Interface Device Deduplication**: Prevents duplicate peer cards when a single device is reachable over both Wi-Fi and Ethernet. Seamlessly updates existing cards to the preferred interface IP.
- **Selection Preservation**: Preserves user's actively selected target peer in the UI even when the peer transitions between network interfaces or updates its IP.

### Technical Implementation
- **Rust Backend**:
  - `src-tauri/src/discovery/nic.rs`: Adapter scoring and `get_candidate_subnets()` resolution.
  - `src-tauri/src/discovery/udp.rs`: `PeerTracker` deduplication by `device_name` with interface priority resolution.
  - `src-tauri/src/pairing/scanner.rs`: `sweep_all_subnets_for_pin()` multi-subnet parallel sweep.
  - `src-tauri/src/commands.rs`: Connected `connect_by_pin` to multi-subnet sweep.
- **Frontend**:
  - `src/script/peers.js`: `getPeerIdentityKey()`, deduplicated `setPeerList`, `upsertPeer`, and selection preservation.
- **Tests**:
  - `src-tauri/tests/discovery_test.rs` and `src-tauri/tests/scanner_test.rs` for Rust discovery and sweep logic.
  - `tests/frontend_peers_dedup.test.js` for frontend peer identity deduplication and selection preservation.

---

## 7. Single-Instance Application Enforcement & Window Focus Handshake

### Overview
Guarantees that only a single instance of Easy Share runs at any given time, preventing port binding collisions, duplicate network heartbeats, and user confusion when the app is minimized to the system tray.

### Delivered Capabilities
- **Duplicate Process Termination**: Secondary launch attempts detect the existing running process and terminate immediately.
- **Window Restore & Focus Handshake**: When a second instance attempt occurs, the primary instance automatically unhides/shows, unminimizes, and brings the existing window to the foreground.
- **Tray-Aware Restoration**: Shared `restore_main_window` helper seamlessly un-hides the app whether launched from the system tray menu or desktop shortcut.

### Technical Implementation
- **Rust Backend**: Integrated `tauri-plugin-single-instance` (v2) in `src-tauri/Cargo.toml` and registered in `tauri::Builder` chain in `src-tauri/src/lib.rs`; centralized window restore logic in `src-tauri/src/tray.rs`.

---

# Part 2: Upcoming Roadmap Candidates

The following features represent prospective roadmap items under consideration for future releases.

---

## 1. Transfer Queue & Multi-Peer Broadcast

### Overview
Enable sending batches to multiple peers simultaneously or managing an orderly queue of incoming/outgoing transfers.

### Key Capabilities
- **Multi-Peer Selection**: Select multiple detected devices from the "Nearby devices" aside and start concurrent or sequential transfers.
- **Transfer Queue Manager**: Queue transfers if a peer is already busy or if network bandwidth is constrained.
- **Priority Reordering & Pause-Per-Peer**: Pause or cancel specific peers within a multi-target transfer independently.

### Technical Scope
- **Rust Backend**: Expand `AppState` to track multi-session registries mapped by `(peer_id, session_token)`.
- **Frontend**: Multi-select peer toggle in UI; queue status drawer showing pending, active, and completed transfer batches.

---

## 2. Automatic Peer Discovery over Local Bluetooth / BLE (Bypass Strict LANs)

### Overview
Allow devices to exchange initial pairing PINs and connection endpoints over Bluetooth Low Energy (BLE) advertisements when connected to isolated networks where all IP broadcast/multicast and sweep traffic is strictly firewalled.

### Key Capabilities
- **BLE Beaconing**: Advertise Easy Share pairing payload via BLE peripheral mode.
- **Zero-Config Pairing**: Automatically discover nearby peers even when devices are on separate VLANs or AP isolation blocks subnet sweeps.
- **Direct Wi-Fi Direct / Hotspot Handoff**: Fall back to an ad-hoc local hotspot if no common network exists.
