# Easy Share — Feature Roadmap & Specifications (`newFeatures.md`)

This document outlines the planned feature additions, enhancements, and architectural specifications for **Easy Share (v2.x)** on desktop platforms (Windows, macOS, Linux).

---

## 1. Storage & Download Management

### 1.1 Custom Download Directory Picker ✅ [Completed]
- **Current State:** Implemented in `v2.1`. Users can configure any directory as their download destination.
- **Implementation:**
  - Settings modal with native folder picker (`pick_folder`) and path verification.
  - Backend command `set_download_dir(path: String)` with directory validation, runtime update of `ServerState.download_dir`, and JSON persistence (`settings.json`).
  - Runtime and startup restoration of custom directory across app restarts.

### 1.2 "Open Downloads Folder" Shortcut ✅ [Completed]
- **Current State:** Implemented in `v2.1`.
- **Implementation:**
  - Added "Downloads" folder button in top header bar with direct trigger to `open_download_dir`.
  - Added "Open Folder" quick action inside Settings modal.
  - Added prompt hint on incoming file completion toast.

---

## 2. Transfer History & Activity Log

### 2.1 Persistent Transfer Log
- **Target Specification:**
  - Create a dedicated **Transfer History** tab or slide-out drawer accessible from the top navigation bar.
  - Track both **Sent** and **Received** batches with the following metadata:
    - Transfer timestamp (date & time)
    - Direction (`Sent` / `Received`)
    - Remote peer identity (device name, IP address, OS)
    - File details (file count, individual file names, individual sizes, total batch size)
    - Transfer outcome (`Completed`, `Cancelled`, `Failed`, with error reason if failed)
    - Local file path on disk (for received files)

### 2.2 Direct File Actions from History
- For completed incoming transfers, each history item provides quick action buttons:
  - **Show in Folder:** Selects and highlights the file inside the OS file manager.
  - **Open File:** Launches the file with the default system application.
- Provide a "Clear History" button and an optional search/filter bar for past transfers.

---

## 3. Folder Transfers & Directory Structure Preservation

### 3.1 Preserving Relative Directory Hierarchy
- **Current State:** Folder selection or multi-file drops flatten files into a flat list, and the receiver strips path components, dumping all files into the root download folder with naming collisions.
- **Target Specification:**
  - Add a `relative_path` field to `FileMetadata` in both Rust backend and frontend transfer models.
  - On the sending side, recursively index directory trees while retaining each file's relative path from the selected root folder (e.g., `project/src/index.js`).
  - On the receiving side, sanitize against directory traversal attacks (prevent `../` or absolute path escapes) and recreate the directory tree automatically before streaming chunks into the destination files.

### 3.2 "Add Folder" Picker & Staging Polish
- **Target Specification:**
  - Add an explicit **"Add Folder"** button alongside the current "Add Files" / browse trigger, calling `pick_folder`.
  - Add a **"Clear All"** button in the staging area to reset all staged files in one click.
  - Expand drag-and-drop detection so dragging files anywhere over the main app window activates an overlay dropzone rather than only targeting the compact dashed container.

---

## 4. Real-time Metrics & Transfer Controls

### 4.1 Speed (MB/s) and Time Remaining (ETA)
- **Target Specification:**
  - In addition to percentage progress (`Sending (45%)`), compute and display:
    - **Current Transfer Speed:** Rolling average throughput in `KB/s` or `MB/s` (updated every 250–500ms).
    - **Data Transferred:** Bytes transferred vs total batch size (e.g., `450 MB / 1.2 GB`).
    - **Estimated Time Remaining (ETA):** Calculated from remaining bytes and current throughput (e.g., `~18s remaining`).
  - Show these metrics on both the sender card and the receiver's progress drawer.

### 4.2 Pause and Resume Transfer Controls
- **Target Specification:**
  - Expose interactive **Pause** and **Resume** buttons next to Cancel during active transfers.
  - Utilize the existing HTTP chunk streaming and byte-offset protocol (`X-File-Offset` / `Range`) to suspend chunk reads on the sender and seamlessly continue from the last confirmed byte without restarting from zero.

---

## 5. Device Identity, Pairing & Settings Panel

### 5.1 Settings Modal (Header Gear Icon) ✅ [Completed - Foundation]
- **Current State:** Implemented in `v2.1`. Accessible via top header "Settings" button.
- **Implementation:**
  - Settings Modal dialog with keyboard access (Escape dismissal, Enter submit).
  - Groups:
    - **Device Identity:** Name input with 1–32 char limit and immediate UI badge sync.
    - **Download Destination:** Active path display, "Change" folder picker button, and "Open Folder" action.
  - Planned extensions: Themes, Network Interface (NIC) selector, Trusted Devices.

### 5.2 Custom Device Display Name ✅ [Completed]
- **Current State:** Implemented in `v2.1`.
- **Implementation:**
  - Live editing with input validation (trimmed, length checks).
  - Backend command `set_device_name(name: String)` updates `ServerState.device_info`, emits `device-info-updated` event, and persists to `settings.json`.
  - Startup restoration of custom name across restarts.

### 5.3 Trusted Devices & Auto-Accept
- **Target Specification:**
  - Add an **"Always allow transfers from this device"** (Trust device) checkbox in the incoming consent modal.
  - Maintain a list of trusted peer fingerprints/IDs in settings.
  - When an incoming transfer arrives from an authenticated trusted peer, automatically accept and start downloading without requiring manual user interaction each time.

---

## 6. Desktop Integration & Productivity

### 6.1 Native OS Desktop Notifications
- **Target Specification:**
  - Send native operating system notifications (via Tauri Notification plugin):
    - When an incoming transfer request arrives while the window is minimized or in the background.
    - When a large batch transfer completes successfully or fails.
  - Clicking the notification brings Easy Share to the foreground or opens the received file folder.

### 6.2 Quick Text & Clipboard Sharing
- **Target Specification:**
  - Add a "Share Text / Link" toggle or tab in the transfer workspace.
  - Allows sending clipboard contents, URLs, or notes directly to a peer without having to create and save a `.txt` file first.
  - On the receiving device, shows a card with the text and a one-click "Copy to Clipboard" button.

### 6.3 System Tray & Background Operation
- **Target Specification:**
  - Add a system tray icon with a context menu (`Open Easy Share`, `Open Downloads`, `Pause Discovery`, `Quit`).
  - Allow the app to minimize to tray when closed or minimized, keeping the local P2P server alive to receive incoming files in the background.
