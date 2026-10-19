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

## 2. End-to-End Encryption (E2EE) with PIN-Based Handshake

### Overview
Upgrade local HTTP/TCP socket transfers with authenticated end-to-end encryption to prevent eavesdropping on untrusted or shared Wi-Fi networks.

### Key Capabilities
- **Authenticated Key Exchange**: Derive ephemeral symmetric encryption keys using the existing 4-digit pairing PIN and Diffie-Hellman / HKDF exchange.
- **Authenticated Encryption**: Encrypt streaming chunks in-flight using modern ciphers like **ChaCha20-Poly1305** or **AES-256-GCM**.
- **Integrity & Verification**: Guarantee tamper-proof chunk delivery and authentication without relying on external certificate authorities.
- **Security Badge**: Visual badge on the transfer card indicating encrypted transfer status.

### Technical Scope
- **Rust Backend**: Integrate `ring` or `chacha20poly1305` in `src-tauri`; update sender streaming loop and receiver chunk endpoint to encrypt/decrypt buffers before writing.
- **Frontend**: Display an "E2EE Secured" padlock badge on peer cards and the active progress cards.

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

## 4. Bandwidth Throttling / Speed Limiter

### Overview
Provide user-configurable transfer speed caps to prevent saturating the local network during large file transfers.

### Key Capabilities
- **Global / Per-Transfer Presets**: Selectable speeds: 5 MB/s, 10 MB/s, 25 MB/s, 50 MB/s, or Unlimited.
- **On-the-Fly Adjustment**: Adjust speed limits directly from the active `#sendProgressCard` slider or dropdown without interrupting the transfer.
- **Settings Default**: Configurable default cap in the Settings view.

### Technical Scope
- **Rust Backend**: Token bucket / rate-limiter in `sender.rs` chunk loop (e.g. `governor` or manual sleep calculation based on EWMA tracker).
- **Frontend**: Add a speed limiter dropdown/control in Settings and dynamically on `#sendProgressCard`.

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
