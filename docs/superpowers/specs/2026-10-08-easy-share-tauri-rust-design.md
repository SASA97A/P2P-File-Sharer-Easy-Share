# Easy Share (Rust + Tauri 2.0) Architecture & Wire Protocol Specification

**Date:** 2026-10-08  
**Status:** Approved for Implementation Planning  
**Target Platform Priority:** Windows (Primary) $\rightarrow$ Linux / macOS (Secondary) $\rightarrow$ Android / iOS (Future Expansion)

---

## 1. Executive Summary & Goals

Easy Share is being re-architected from Electron/Node.js into a high-performance, lightweight **Tauri 2.0 + Rust** desktop/mobile application. The new architecture replaces raw ad-hoc TCP sockets with an embedded asynchronous HTTP/1.1 transfer engine built on **Tokio + Axum**, guaranteeing:

1. **High-Speed, Large-File Streaming:** Sustained Gigabit/10GbE network saturation for files from a few KB up to 100+ GB without UI stutter or garbage collection spikes.
2. **Resumable Transfers:** Partial transfer persistence (`.part` files) and byte-offset resumption if the network is interrupted.
3. **Explicit Consent & Accept/Reject Handshake:** Receivers receive an interactive prompt with sender information, file counts, and total payload size before any file bytes are written to disk.
4. **Room / Group / Pairing Foundation:** Extensible peer metadata supporting room-based filtering, group transfers, and pre-shared PIN authentication.
5. **Cross-Platform Compatibility:** 100% interoperability across Windows, macOS, Linux, Android, and iOS using native WebViews for UI and unified native Rust for all networking and file I/O.

---

## 2. High-Level System Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                   Frontend UI Layer (Native WebView)                  │
│  - Device Radar / Peer List (with OS icons & Room tags)                │
│  - Drag & Drop Zone + File Picker                                      │
│  - Interactive Accept / Reject Dialog Modal                            │
│  - Per-File Real-time Progress Bars & Speed/ETA Indicator             │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Tauri IPC (Commands & Events)
┌───────────────────────────────────▼────────────────────────────────────┐
│                    Tauri 2.0 Rust Core Engine                          │
│                                                                        │
│  ┌─────────────────────────┐         ┌──────────────────────────────┐  │
│  │   Peer Discovery Core   │         │    Transfer Engine (Axum)    │  │
│  │  - mDNS Engine (mdns-sd)│         │  - Async File Receiver Server│  │
│  │  - UDP Broadcast Socket │         │  - Async File Sender Client  │  │
│  │  - NIC Filter (Physical)│         │  - Resumable Chunk Pipeline  │  │
│  └─────────────────────────┘         │  - BLAKE3 SIMD Checksums     │  │
│                                      └──────────────────────────────┘  │
│  ┌─────────────────────────┐         ┌──────────────────────────────┐  │
│  │   Session & State Core  │         │       File I/O Manager       │  │
│  │  - Active Transfers Map │         │  - Safe Path Sanitization    │  │
│  │  - Accept/Reject State  │         │  - Pre-allocation (NTFS/ext4)│  │
│  │  - Room / PIN Validator │         │  - Atomic .part Rename       │  │
│  └─────────────────────────┘         └──────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Wire Protocol & REST API Specification

Every Easy Share node runs an embedded HTTP server (default port `5050`, falling back to dynamic ports if occupied). All requests and responses use `application/json` for metadata and `application/octet-stream` for file chunk streaming.

### 3.1 Endpoints Overview

| Method | Path | Description |
| :--- | :--- | :--- |
| `GET` | `/api/v1/info` | Health check & device identity probe |
| `POST` | `/api/v1/transfer/request` | Handshake: Sender requests permission to send files |
| `GET` | `/api/v1/transfer/status` | Query byte offset of a partial or pending transfer |
| `POST` | `/api/v1/transfer/chunk` | Stream file binary payload starting at a specific byte offset |
| `POST` | `/api/v1/transfer/finish` | Finalize transfer, verify BLAKE3 checksum, and commit file |
| `POST` | `/api/v1/transfer/cancel` | Abort a transfer session and clean up temp resources |

---

### 3.2 Endpoint Details & Payloads

#### 1. Device Info Probe: `GET /api/v1/info`
Returns device metadata for active probing and validation.
* **Response (200 OK):**
```json
{
  "device_name": "Alice-PC",
  "device_type": "desktop",
  "os": "windows",
  "version": "2.0.0",
  "room_id": "Engineering",
  "port": 5050
}
```

---

#### 2. Transfer Request (Handshake): `POST /api/v1/transfer/request`
Initiates a transfer session. Triggers the receiver's UI Accept/Reject prompt.
* **Request Body:**
```json
{
  "request_id": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
  "sender_name": "Alice-PC",
  "sender_os": "windows",
  "room_id": "Engineering",
  "total_bytes": 1073741824,
  "files": [
    {
      "file_id": "f1-abc",
      "name": "archive.zip",
      "size": 1073741824,
      "blake3_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    }
  ]
}
```
* **Receiver Behavior:**
  * Emits a Tauri event `transfer-requested` to the frontend with file metadata and sender info.
  * Receiver UI displays an explicit dialog: `"Alice-PC wants to send 1 file (1.0 GB). Accept?"`.
  * Sender request remains awaiting response (with a 60-second timeout).
* **Response (200 OK - Accepted):**
```json
{
  "status": "accepted",
  "session_token": "sess-99887766-5544-3322",
  "existing_offsets": {
    "f1-abc": 0
  }
}
```
* **Response (403 Forbidden - Declined):**
```json
{
  "status": "declined",
  "reason": "User rejected the transfer request."
}
```

---

#### 3. Transfer Status Query (For Resumption): `GET /api/v1/transfer/status?session_token=...&file_id=...`
Queries the receiver for the number of bytes already written to disk for a specific file.
* **Response (200 OK):**
```json
{
  "file_id": "f1-abc",
  "bytes_received": 524288000,
  "status": "partial"
}
```

---

#### 4. File Chunk Streaming: `POST /api/v1/transfer/chunk`
Streams binary data directly to disk.
* **Query Parameters:**
  * `session_token`: The token issued during handshake acceptance.
  * `file_id`: Unique file ID within the session.
  * `offset`: The starting byte position (e.g., `0` for new file, `524288000` for resumed file).
* **Headers:**
  * `Content-Type: application/octet-stream`
  * `Content-Length: <chunk_bytes>`
* **Body:** Raw binary byte stream.
* **Response (200 OK):**
```json
{
  "file_id": "f1-abc",
  "bytes_written": 524288000,
  "current_total": 1048576000
}
```

---

#### 5. Transfer Finalization: `POST /api/v1/transfer/finish`
Signals that all bytes have been transmitted. Receiver validates file size and BLAKE3 checksum, then renames `.part` to the final filename.
* **Request Body:**
```json
{
  "session_token": "sess-99887766-5544-3322",
  "file_id": "f1-abc"
}
```
* **Response (200 OK):**
```json
{
  "status": "completed",
  "file_id": "f1-abc",
  "saved_path": "C:\\Users\\User\\Downloads\\archive.zip"
}
```

---

## 4. Resumable File Streaming Engine & I/O Design

### 4.1 Temporary File Convention (`.part`)
1. When a file `sample.iso` is accepted, the receiver creates a working file in `Downloads`:
   `sample.iso.<session_id>.part`
2. If `sample.iso` already exists upon completion, the receiver increments the filename (`sample(1).iso`, `sample(2).iso`) before renaming the `.part` file.
3. If transfer is cancelled or abandoned past a 24-hour expiration window, the `.part` file is cleaned up.

### 4.2 Large File Optimization (10GB–100GB+)
* **Pre-Allocation:** On Windows, Rust calls `SetFileInformationByHandle` (`FileAllocationInfo`) or `SetEndOfFile` when a transfer starts to allocate contiguous disk space and eliminate NTFS fragmentation.
* **Bounded Chunk Streaming:** File reads and socket streaming use a multi-buffer pool (64 KB to 4 MB chunks) without buffering entire files in memory.
* **Fast BLAKE3 Hashing:** Hashes are computed incrementally as chunks are read/written using SIMD instructions (AVX2/AVX-512/NEON), reaching hashing speeds over 4 GB/s.

---

## 5. Peer Discovery Engine

### 5.1 Dual Discovery Strategy
1. **mDNS (Multicast DNS via `mdns-sd`):**
   * Service Type: `_easyshare._tcp.local.`
   * TXT Records: `name=<DeviceName>`, `os=<OS>`, `type=<desktop|mobile>`, `room=<RoomId>`, `v=2.0.0`
2. **UDP Broadcast (Fallback for blocked mDNS subnets):**
   * Port: `41234` (with `SO_REUSEADDR` / `SO_REUSEPORT` enabled)
   * Broadcast interval: 5 seconds.
   * Format: JSON heartbeat payload.

### 5.2 Network Interface Filtering (Preventing Virtual NIC Traps)
* Sockets inspect available interfaces using `local_ip_address` / `ipnetwork`.
* Virtual adapters (e.g. `vEthernet`, `WSL`, `Docker0`, `VMware`, `Tailscale`, `169.254.x.x` APIPA) are filtered out, prioritizing physical 802.11 Wi-Fi and Ethernet adapters on `192.168.0.0/16`, `10.0.0.0/8`, and `172.16.0.0/12`.

---

## 6. Room, Group, & PIN Security Architecture

1. **Room Segregation:**
   * Peers can set an active `room_id` (e.g. `"Design Team"`, `"Home"`, or default `"Public"`).
   * Discovery lists peers grouped by room, filtering out unmatched rooms if "Private Room Mode" is enabled.
2. **Path Traversal Sanitization:**
   * Receiver strictly extracts `path::Path::new(file.name).file_name()`.
   * Rejects any path containing separators (`/`, `\`), null bytes, or Windows reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`).
3. **Session Token Isolation:**
   * Each transfer handshake generates a cryptographically random UUIDv4 `session_token`.
   * Chunk writes are only permitted with a valid, active token for the matching sender IP.

---

## 7. Frontend UI & UX Architecture

* **Framework:** Modern Vanilla JS + CSS (or lightweight Vite/Svelte) inside Tauri Webview.
* **Key Components:**
  * **Device Radar:** Interactive grid displaying discovered peers with device icons (🖥️ Windows/Mac/Linux, 📱 Android/iOS), room tags, and persistent selection state.
  * **File Staging Area:** Multi-file list with drag-and-drop support, individual file size calculation, and total payload badges.
  * **Consent Modal:** Clean overlay dialog displaying incoming transfer details (Sender name, IP, list of files, total size) with `Accept` and `Decline` actions.
  * **Active Transfer Dashboard:** Real-time progress bar per file, transfer speed (MB/s), remaining ETA, and pause/cancel/resume controls.
  * **Toast Feedback:** Non-blocking alerts for network events, completed transfers, and saved file paths.

---

## 8. Phased Platform Roadmap

* **Phase 1 (Windows Desktop MVP - Immediate Priority):**
  * Tauri 2.0 scaffold with Rust backend (`axum`, `tokio`, `mdns-sd`).
  * Full protocol implementation, resumability, Accept/Reject dialog, and modern UI.
* **Phase 2 (macOS & Linux Desktop):**
  * Platform-specific downloads directory resolution (`dirs` crate).
  * WebKitGTK / WKWebView validation and Linux firewall documentation.
* **Phase 3 (Mobile - Android & iOS):**
  * Android Storage Access Framework & WiFi MulticastLock integration.
  * iOS Local Network Usage permission descriptors in `Info.plist`.

---

## 9. Verification & Acceptance Criteria

1. **Functional Transfer:** Successfully transfers files of varying sizes (1 KB, 100 MB, 10 GB+) between two application instances on the local network.
2. **Resumption Verification:** Manually interrupting a transfer mid-stream (closing socket / disabling network) and resuming completes without data corruption, verified by BLAKE3 hash match.
3. **Consent Enforcement:** Senders cannot write data to receiver disks without the receiver clicking "Accept" in the UI dialog.
4. **Performance & Memory:** RAM usage remains under 50 MB during a sustained 20 GB file transfer; CPU utilization remains minimal.
5. **Security Check:** Malicious file names (e.g. `../../test.exe`) are sanitized and safely stored in the designated Downloads directory.
