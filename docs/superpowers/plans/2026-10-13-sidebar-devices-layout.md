# Implementation Plan: Sidebar Devices Layout & Local PC Card

We are redesigning the Easy Share desktop UI layout so that nearby devices are displayed as a vertical sidebar list on the right side, with the local PC name pinned at the top of the sidebar.

## Proposed Changes

### 1. `src/index.html`
- Update `.container.app-shell`:
  - Retain `.app-header` with Title, Subtitle, and `#openPairingBtn`.
  - Wrap the body content inside a `.main-layout` grid container:
    - **Left column (`.transfer-workspace`):**
      - `.file-section` (`#fileList`, `#drop-area`)
      - `#sendBtn` (Send files button)
    - **Right column (`.devices-sidebar`):**
      - `.local-device-section` containing a heading ("This Device") and `#myDeviceBadge` (`#myDeviceIcon`, `#myDeviceName`, `#myDeviceStatus`).
      - `.device-section` containing heading ("Nearby Devices" + `#deviceCount`) and `#deviceList`.

### 2. `src/styles/styles.css`
- Update container sizing: increase `.container` max-width to `960px` or full flex width.
- Add `.main-layout` grid layout: `grid-template-columns: 1fr 310px; gap: 20px; align-items: start;`.
- Add responsive breakpoint for `<768px` to stack `.main-layout` in a single column.
- Style `.devices-sidebar`: border, background `#fafaf6`, border-radius `18px`, padding `16px`, flex column.
- Style `.local-device-card` / `#myDeviceBadge` inside the sidebar with clean border, pill icon, and online status.
- Style `.device-list` to be a vertical stack (`flex-direction: column; gap: 8px; max-height: 380px; overflow-y: auto;`).
- Style `.device` items as horizontal row cards: icon on left, name/meta in center, checkmark on right.
- Ensure `#sendBtn` spans the workspace width appropriately.

### 3. `src/script/ui.js`
- Update `renderPeers` to wrap `device-name` and `device-meta` in a `device-details` container for clean row layout, with text truncation support.

## Tasks

- [ ] Task 1: Restructure HTML in `src/index.html`
- [ ] Task 2: Implement sidebar & row CSS in `src/styles/styles.css`
- [ ] Task 3: Update `renderPeers` markup in `src/script/ui.js`
- [ ] Task 4: Verify build, run test suite, and check visual responsive behavior
