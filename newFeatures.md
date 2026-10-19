# Easy Share - Planned Features & Roadmap Candidates

This document outlines potential feature candidates for Easy Share following the app shell redesign and real-time metrics/controls integration. Review the options below to decide the next development priority.

---

## 1. Directory Tree Preservation for Folder Transfers [COMPLETED]

### Overview
Allow users to select or drop entire directory structures and recreate the complete hierarchy on the receiving peer, rather than flattening files into the download directory.

### Key Capabilities
- **Relative Path Manifest**: Send relative subpaths alongside file payloads in `TransferRequestPayload` (e.g., `project/src/main.rs`).
- **Receiver Folder Construction**: Automatically create intermediate directories in the receiver's target download folder before streaming chunks.
- **Visual Staging**: Group staged files under expandable folder headers in the workspace staging list.
- **Collision Avoidance**: Handle duplicate folder names cleanly (e.g., auto-renaming `MyFolder (1)` or prompting for overwrite).

### Technical Scope
- **Rust Backend**: Extend `commands.rs` inspect helper to preserve relative folder paths; adjust `routes.rs` write destination to sanitize subpaths and create parent dirs via `tokio::fs::create_dir_all`.
- **Frontend**: Update `files.js` and `transfer.js` to track `relative_path` attributes for staged items.

---

## 2. End-to-End Encryption (E2EE) with PIN-Based Handshake [COMPLETED]

### Overview
Upgrade local HTTP/TCP socket transfers with authenticated end-to-end encryption to prevent eavesdropping on untrusted or shared Wi-Fi networks.

### Key Capabilities
- **Authenticated Key Exchange**: Derive ephemeral symmetric encryption keys using the existing pairing PIN and HKDF-based key derivation.
- **Authenticated Encryption**: Encrypt streaming chunks in-flight using **ChaCha20-Poly1305** AEAD encryption.
- **Integrity & Verification**: Guarantee tamper-proof chunk delivery and authentication without external certificate authorities.
- **Security Settings & Toggle**: User-configurable toggle in Settings view with persistent state.

### Technical Scope
- **Rust Backend**: Implemented in `src-tauri/src/crypto.rs` using ChaCha20-Poly1305; integrated into `TransferClient`, `sender.rs`, and server chunk streaming routes (`routes.rs`); commands exposed in `commands.rs`.
- **Frontend**: Settings toggle in `settings.js` with persistence via Tauri bridge.

---

## 3. Transfer Queue & Multi-Peer Broadcast

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

## 4. Bandwidth Throttling / Speed Limiter [COMPLETED]

### Overview
Provide user-configurable transfer speed caps to prevent saturating the local network during large file transfers.

### Key Capabilities
- **Global / Per-Transfer Presets**: Selectable speeds: 5 MB/s, 10 MB/s, 25 MB/s, 50 MB/s, or Unlimited.
- **On-the-Fly Adjustment**: Adjust speed limits directly from the active `#sendProgressCard` slider or dropdown without interrupting the transfer.
- **Settings Default**: Configurable default cap in the Settings view.

### Technical Scope
- **Rust Backend**: Token bucket / rate-limiter in `sender.rs` chunk loop via atomic speed limit tracker (`AtomicU64`); IPC commands `get_speed_limit` and `set_speed_limit` in `commands.rs`.
- **Frontend**: Speed limiter dropdown and control in Settings and transfer progress card (`settings.js`, `transfer.js`, `metrics.js`).

---

## 5. System Tray & Native OS Desktop Notifications [COMPLETED]

### Overview
Allow Easy Share to run smoothly in the background, minimizing to the system tray and delivering OS notifications on transfer events.

### Key Capabilities
- **System Tray Integration**: Minimize to tray on close, tray menu with "Open", "Pause All", "Clear Finished", and "Quit".
- **Native Notifications**: Desktop notifications for incoming transfer requests (with quick Accept/Decline action buttons), transfer complete, and connection errors.
- **Background Auto-Receive Option**: Optional setting to auto-accept transfers from previously paired devices when running in the tray.

### Technical Scope
- **Rust Backend**: Configure Tauri system tray plugin (`tauri::tray`) and notification API (`tauri-plugin-notification`).
- **Frontend**: Add notification and minimize-to-tray toggles to the Settings view (`src/script/settings.js`).

---

## 6. Native Tauri 2 Drag-and-Drop Staging with Directory Hierarchy [COMPLETED]

### Overview
Deliver native operating system drag-and-drop file and directory staging into the Easy Share window, bypassing browser webview file sandbox limitations and preserving full relative directory hierarchies.

### Key Capabilities
- **Tauri 2 Window Drop Integration**: Captures native window file drops using `getCurrentWebviewWindow().onDragDropEvent` across Windows, macOS, and Linux.
- **Deep Recursive Directory Inspection**: Recursively unpacks folders dropped into the window, extracting metadata (relative paths, sizes, item counts) in background threads.
- **Staging Area Visual Feedback**: Active drag hover styling and dynamic file/folder counter staging cards.
- **Dual Fallbacks**: Robust fallback chain supporting `getCurrentWebviewWindow().onDragDropEvent`, legacy window event bridges, mock event bus for testing, and DOM DataTransfer fallback.

### Technical Scope
- **Rust Backend**: `inspect_file_paths` command in `src-tauri/src/commands.rs` recursively walks folder paths and preserves relative folder hierarchy.
- **Frontend**: Tauri drag-drop handler in `src/script/tauri-bridge.js` and window event routing in `src/script/renderer.js` and `src/script/files.js`.
- **Testing**: End-to-end unit test suite in `tests/frontend_drag_drop.test.js`.

---

## 7. Multi-Interface Subnet Scanning, Ethernet Prioritization & Device Deduplication [COMPLETED]

### Overview
Enable seamless multi-network peer discovery and PIN pairing across both Wi-Fi and Ethernet interfaces, prioritizing high-speed wired Ethernet connections for file transfers while avoiding duplicate peer cards in the UI.

### Key Capabilities
- **Physical Ethernet Prioritization**: Interface scoring awards physical Ethernet adapters (+50) higher priority over Wi-Fi (+20), ensuring high-speed LAN links are used whenever available.
- **Deduplicated Multi-Subnet Sweeps**: Discovers and scans unique `/24` subnets across all active host interfaces (Ethernet first, then Wi-Fi) with immediate short-circuiting upon PIN match.
- **Cross-Interface Device Deduplication**: Prevents duplicate peer cards when a single device is reachable over both Wi-Fi and Ethernet. Seamlessly updates existing cards to the preferred interface IP.
- **Selection Preservation**: Preserves user's actively selected target peer in the UI even when the peer transitions between network interfaces or updates its IP.

### Technical Scope
- **Rust Backend**:
  - `src-tauri/src/discovery/nic.rs`: Adapter scoring and `get_candidate_subnets()` resolution.
  - `src-tauri/src/discovery/udp.rs`: `PeerTracker` deduplication by `device_name` with interface priority resolution.
  - `src-tauri/src/pairing/scanner.rs`: `sweep_all_subnets_for_pin()` multi-subnet parallel sweep.
  - `src-tauri/src/commands.rs`: Connected `connect_by_pin` to multi-subnet sweep.
- **Frontend**:
  - `src/script/peers.js`: `getPeerIdentityKey()`, deduplicated `setPeerList`, `upsertPeer`, and selection preservation.
- **Testing**:
  - `src-tauri/tests/discovery_test.rs` and `src-tauri/tests/scanner_test.rs` for Rust discovery and sweep logic.
  - `tests/frontend_peers_dedup.test.js` for frontend peer identity deduplication and selection preservation.
