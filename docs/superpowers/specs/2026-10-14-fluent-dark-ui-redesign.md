# Design Spec: Fluent Dark UI Redesign for Easy Share

- **Date:** 2026-10-14
- **Branch:** `feat/fluent-dark-ui`
- **Target:** `rustRewrite`
- **Reference Mockup:** `docs/superpowers/New Textdokument.txt`

---

## 1. Executive Summary & Goals

Transform Easy Share's desktop user interface into a refined, high-craft dark theme inspired by modern Fluent Design and developer utilities (e.g., Linear, Raycast, Windows 11).

Key goals:
- **Dark Theme Default:** Replace the light palette with deep obsidian canvas surfaces (`#0e0e0e`), elevated containers (`#1a1a1a`), and subtle outline borders (`#333333`).
- **Warm Designer Accent:** Adopt the warm copper/amber accent (`#ffb77b` / `#ffa255`) for active buttons, progress indicators, and focal points.
- **Offline-First Lucide Icons:** Render crisp, bundled Lucide SVGs with full offline reliability without external CDN font dependencies.
- **Precision Telemetry Typography:** Utilize `JetBrains Mono` / `Cascadia Code` with `font-variant-numeric: tabular-nums` for all dynamic data (IP addresses, transfer speeds, remaining times, byte sizes, and PINs).
- **Two-Column Desktop Hierarchy:** Maintain the left transfer workspace and right devices sidebar (with "This Device" pinned at top).

---

## 2. Visual Token Architecture

### 2.1 CSS Variables & Palette (`src/styles/styles.css`)

```css
:root {
  /* Surfaces & Canvas */
  --color-bg: #0e0e0e;
  --color-surface-container: #1a1a1a;
  --color-surface-container-high: #242424;
  --color-surface-container-highest: #2e2e2e;
  --color-surface-container-lowest: #0a0a0a;

  /* Outlines & Borders */
  --color-border: #2a2a2a;
  --color-border-subtle: #1f1f1f;
  --color-border-variant: #3e484f;

  /* Typography Colors */
  --color-text-main: #e5e2e1;
  --color-text-muted: #9ca3af;
  --color-text-subtle: #6b7280;

  /* Accents */
  --color-primary: #ffb77b;
  --color-primary-hover: #ffa255;
  --color-primary-dim: rgba(255, 183, 123, 0.15);
  --color-primary-glow: rgba(255, 183, 123, 0.35);

  /* Semantic Feedback */
  --color-success: #10b981;
  --color-success-dim: rgba(16, 185, 129, 0.15);
  --color-error: #ffb4ab;
  --color-error-container: #93000a;
  --color-info: #38bdf8;

  /* Typography Stacks */
  --font-sans: "Segoe UI Variable", "Segoe UI", -apple-system, BlinkMacSystemFont, Roboto, sans-serif;
  --font-mono: "JetBrains Mono", "Cascadia Code", ui-monospace, Menlo, Consolas, monospace;

  /* Geometry & Shadows */
  --radius-sm: 8px;
  --radius-md: 12px;
  --radius-lg: 16px;
  --radius-pill: 999px;
  --shadow-elevation: 0 8px 24px rgba(0, 0, 0, 0.45);
  --shadow-modal: 0 24px 64px rgba(0, 0, 0, 0.7);
}
```

---

## 3. Component Specifications

### 3.1 App Header
- **Branding:** App icon, "Easy Share" title with subtle subtitle ("Local P2P File Transfer").
- **Header Actions:** Quick-access "Pair Device" pill button with dark elevated background, subtle border, hover lift, and focus-visible ring.

### 3.2 Left Column: Transfer Workspace
- **Dropzone (`.drop-area`):**
  - Dark container `#1a1a1a` with 1.5px dashed border `--color-border-variant`.
  - Icon with warm amber tint.
  - Hover / Drag-over state: glowing `--color-primary-glow` border with subtle surface lift.
- **Staged File List (`#selectedFilesList`):**
  - Counter badge: "Staged Files (N)" and summary size.
  - File rows:
    - Format-specific icon badge with low-saturation colored background (e.g. `.pdf` in coral, media in blue, archives in emerald, general in zinc).
    - Truncated filename in crisp white `--color-text-main`.
    - Tabular file size in `--font-mono`.
    - Ghost remove button (`✕`) with hover red tint.
- **Transfer Progress & Action Footer:**
  - Progress bar with smooth transitions, amber fill, and subtle shimmer during active transfer.
  - Monospace telemetry metrics: throughput speed (e.g. `45.2 MB/s`), percentage (`68%`), and transfer size (`120 MB / 176 MB`).
  - Primary "Send" button with high-contrast amber background, bold typography, and accessible keyboard focus ring.

### 3.3 Right Column: Devices Sidebar
- **Pinned Top Section ("THIS DEVICE"):**
  - Dedicated card with desktop/laptop icon.
  - Hostname title with monospace local IP badge.
  - "Online" status with glowing green beacon dot (`pulseDot` animation).
- **Nearby Devices List:**
  - Header with device count badge (`N devices found`).
  - Peer cards:
    - OS badge / icon.
    - Peer device name.
    - Monospace IP address.
    - Selected state: amber outline ring (`2px solid var(--color-primary)`) with subtle amber tint background.
  - Empty state: "Searching for nearby devices…" with spinner.

### 3.4 Modals & Dialogs
- **Device Pairing Modal:**
  - Segmented tab bar (PIN | QR Code | Direct IP).
  - PIN tab: Large monospace 6-digit display, countdown progress, and 6-digit input.
  - QR Code tab: High-contrast QR card, instructions, copy pairing link button.
  - Direct IP tab: Monospace IP:port input, connect button, and local address card.
- **Transfer Consent Dialog:**
  - Incoming request card with sender details and file preview list.
  - Decline button (ghost dark) and Accept button (primary amber).
- **Toast Notifications:**
  - Floating dark cards with 1px border and category icons (success, error, info).

---

## 4. Accessibility & Quality Checklist

1. **High Contrast:** All text passes WCAG AA contrast against `#0e0e0e` and `#1a1a1a`.
2. **Keyboard Focus:** Every interactive control has `:focus-visible` ring with `2px solid var(--color-primary)` or `2px solid #ffffff`.
3. **Tabular Figures:** Numbers and metrics use `font-variant-numeric: tabular-nums` to prevent text jitter.
4. **Reduced Motion:** Respect `@media (prefers-reduced-motion: reduce)`.
5. **No Regressions:** Rust backend tests remain 100% passing.
