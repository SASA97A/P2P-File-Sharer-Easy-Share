# Design Spec: Sidebar Devices Layout & Local PC Card

## 1. Overview
Restructure the Easy Share desktop UI into a two-column layout where:
1. The top header remains full-width containing the app brand and the "Pair Device" modal trigger button.
2. The main area is split into:
   - **Left Panel (Transfer Workspace):** Staged file list, drag-and-drop zone, and primary Send button.
   - **Right Panel (Devices Sidebar):** 
     - **Top Section ("This Device"):** Displays local PC identity (name, icon, status badge, network indicator).
     - **Bottom Section ("Nearby Devices"):** Vertical scrollable list of discovered peers rendered as compact horizontal row items.

## 2. Goals & User Experience
- Keep local machine identity prominent at the top of the sidebar.
- Display nearby peers in an ergonomic vertical sidebar list suitable for desktop applications.
- Maintain full responsiveness: smoothly adapt to desktop and narrower window sizes.
- Preserve all existing DOM IDs and event bindings for seamless integration with JavaScript modules (`ui.js`, `peers.js`, `pairing.js`, `transfer.js`).

## 3. Architecture & Structure Changes

### 3.1 DOM Structure (`src/index.html`)
- **Header (`.app-header`):**
  - Left: `.header-left` (Logo/Title "Easy Share" + Subtitle).
  - Right: `#openPairingBtn` ("Pair Device" button).
  - The local identity badge `#myDeviceBadge` moves from `.header-right` into the right sidebar.
- **Main Layout Container (`.main-layout`):**
  - Left Column (`.transfer-workspace`):
    - `.file-section` (`#fileList`, `#drop-area`)
    - `#sendBtn` (Primary transfer action)
  - Right Column (`.devices-sidebar`):
    - `.local-device-section`:
      - Section title: "This Device"
      - `#myDeviceBadge` with `#myDeviceIcon`, `#myDeviceName`, `#myDeviceStatus`, and status dot indicator.
    - `.device-section`:
      - Section header: "Nearby Devices" + `#deviceCount`
      - `#deviceList`: Container for discovered devices and search loader.

### 3.2 CSS Styling (`src/styles/styles.css`)
- **App Shell & Main Layout:**
  - Increase container max-width to `960px` or `1000px` for optimal 2-column spacing.
  - `.main-layout`: `display: grid; grid-template-columns: 1fr 310px; gap: 20px; align-items: start;`
  - Responsive breakpoint (`@media (max-width: 768px)`): Stack `.main-layout` vertically (`grid-template-columns: 1fr;`).
- **Sidebar (`.devices-sidebar`):**
  - Background `#fafaf6`, border `1px solid var(--color-border)`, border-radius `18px`, padding `16px`.
  - Flex column layout with `gap: 18px`.
- **Local Device Card (`.local-device-card` / `#myDeviceBadge`):**
  - Card with white background, subtle border, padding `12px 14px`.
  - Icon container, device name (bold, truncated if too long), status badge ("Online").
- **Device List & Items (`.device-list`, `.device`):**
  - `.device-list`: Vertical flex column (`flex-direction: column; gap: 8px; max-height: 380px; overflow-y: auto;`).
  - `.device`: Flex row (`display: flex; align-items: center; gap: 12px; padding: 10px 12px; width: 100%; border-radius: 12px;`).
  - Left: `.device-icon` (32x32 rounded icon container).
  - Middle: `.device-info` containing `.device-name` and `.device-meta` (IP and OS tag).
  - Right: `.checkmark` selection indicator.

### 3.3 Compatibility & JS Integration
- Ensure all IDs (`myDeviceBadge`, `myDeviceIcon`, `myDeviceName`, `myDeviceStatus`, `deviceList`, `deviceCount`, `sendBtn`, `openPairingBtn`) remain intact.
- Verify `renderPeers` in `src/script/ui.js` renders with classes matching the new row layout.
- Verify keyboard navigation (`tabindex`, `role="button"`, enter/space selection) works identically.

## 4. Verification Plan
- Manual UI check via Tauri dev / browser preview.
- Ensure peer selection updates `#sendBtn` state and checkmark styling correctly.
- Verify responsive layout when window is resized.
- Ensure existing Rust and frontend test suites pass.
