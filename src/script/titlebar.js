/**
 * Easy Share - Custom Title Bar
 * Replaces the native window frame so the app looks the same on every OS.
 * Dragging and double-click to maximize come from the bar's data-tauri-drag-region attribute.
 */

import { getAppWindow } from "./tauri-bridge.js";
import { getIconSvg } from "./icons.js";

const ICON_SIZE = { size: 14 };

/**
 * Wires the window controls. Without a native window (plain browser) they do nothing.
 * @param {Object|null} [appWindow] the Tauri window, resolved automatically when omitted
 */
export async function setupTitlebar(appWindow) {
  const win = appWindow === undefined ? await getAppWindow() : appWindow;
  if (!win) return;

  const minimizeBtn = document.getElementById("windowMinimizeBtn");
  const maximizeBtn = document.getElementById("windowMaximizeBtn");
  const closeBtn = document.getElementById("windowCloseBtn");

  const run = (action) => async () => {
    try {
      await action();
    } catch (err) {
      console.error("Window control failed:", err);
    }
  };

  minimizeBtn?.addEventListener("click", run(() => win.minimize()));
  maximizeBtn?.addEventListener("click", run(() => win.toggleMaximize()));
  // Goes through the app's close handling, which respects "Keep running in the tray"
  closeBtn?.addEventListener("click", run(() => win.close()));

  const syncMaximizeButton = run(async () => {
    if (!maximizeBtn) return;
    const maximized = await win.isMaximized();
    const label = maximized ? "Restore" : "Maximize";
    maximizeBtn.innerHTML = getIconSvg(maximized ? "windowRestore" : "windowMaximize", ICON_SIZE);
    maximizeBtn.title = label;
    maximizeBtn.setAttribute("aria-label", label);
  });

  await syncMaximizeButton();
  await win.onResized(syncMaximizeButton);
}
