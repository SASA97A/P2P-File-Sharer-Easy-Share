/**
 * Easy Share - Settings Controller
 * Handles user preferences, custom download directory, and device display name.
 */

import {
  getMyDeviceInfo,
  setDeviceName,
  getDownloadDir,
  setDownloadDir,
  openDownloadDir,
  pickFolder,
  onDeviceInfoUpdated,
  getSpeedLimit,
  setSpeedLimit,
  getTraySettings,
  setTraySettings,
  getEncryptionSettings,
  setEncryptionSettings,
  getPairedDevicesStatus,
  setRememberPairedDevices,
  forgetPairedDevices,
} from "./tauri-bridge.js";
import { showToast } from "./ui.js";

import { showView, getCurrentView } from "./nav.js";

/**
 * Shows the settings page and populates current configuration values.
 */
export async function openSettingsView() {
  showView("settings");

  try {
    const devInfo = await getMyDeviceInfo();
    const nameInput = document.getElementById("settingsDeviceNameInput");
    if (nameInput && devInfo) {
      nameInput.value = devInfo.device_name || "";
      nameInput.dataset.saved = nameInput.value;
      syncSaveNameVisibility();
    }

    const downloadDir = await getDownloadDir();
    const dirDisplay = document.getElementById("settingsDownloadDirDisplay");
    if (dirDisplay) {
      dirDisplay.textContent = downloadDir || "Default Downloads";
      dirDisplay.title = downloadDir || "";
    }

    const speedLimit = await getSpeedLimit();
    const speedSelect = document.getElementById("settingsSpeedLimitSelect");
    if (speedSelect) {
      speedSelect.value = String(speedLimit || 0);
    }

    const traySettings = await getTraySettings();
    const minToggle = document.getElementById("settingsMinimizeToTrayToggle");
    if (minToggle) {
      minToggle.checked = Boolean(traySettings.minimizeToTray);
    }
    const notifToggle = document.getElementById("settingsNotificationsToggle");
    if (notifToggle) {
      notifToggle.checked = Boolean(traySettings.notificationsEnabled);
    }

    const encryptionEnabled = await getEncryptionSettings();
    const encToggle = document.getElementById("settingsEncryptionToggle");
    if (encToggle) {
      encToggle.checked = Boolean(encryptionEnabled);
    }

    renderPairedDevicesStatus(await getPairedDevicesStatus());
  } catch (err) {
    console.error("Failed to load settings data:", err);
  }
}

/**
 * Renders the "Remember paired devices" row from the backend status.
 * @param {{remember: boolean, paired_count: number, accepted_count: number}} status
 */
function renderPairedDevicesStatus(status) {
  if (!status) return;
  const toggle = document.getElementById("settingsRememberPairedToggle");
  const desc = document.getElementById("settingsRememberPairedDesc");
  const forgetBtn = document.getElementById("settingsForgetPairedBtn");
  // Pairings this device made plus pairings other devices made with it
  const count = (Number(status.paired_count) || 0) + (Number(status.accepted_count) || 0);

  if (toggle) toggle.checked = Boolean(status.remember);
  if (desc) {
    desc.innerHTML = count
      ? `Skip the PIN next time · <span class="mono">${count}</span> ${count === 1 ? "pairing" : "pairings"}`
      : "Skip the PIN next time. Pairings are kept after the app restarts.";
  }
  if (forgetBtn) forgetBtn.classList.toggle("hidden", count === 0);
}

/**
 * Shows the device-name Save button only while the input differs from the saved name.
 */
function syncSaveNameVisibility() {
  const nameInput = document.getElementById("settingsDeviceNameInput");
  const saveBtn = document.getElementById("saveDeviceNameBtn");
  if (!nameInput || !saveBtn) return;
  saveBtn.hidden = nameInput.value.trim() === (nameInput.dataset.saved || "");
}

/**
 * Leaves the settings page and returns to the Send view.
 */
export function closeSettingsView() {
  showView("send");
}

/**
 * Initializes settings listeners and event handlers.
 */
export function setupSettings() {
  const openBtn = document.getElementById("openSettingsBtn");
  const saveNameBtn = document.getElementById("saveDeviceNameBtn");
  const nameInput = document.getElementById("settingsDeviceNameInput");
  const changeDirBtn = document.getElementById("changeDownloadDirBtn");
  const openDirBtn = document.getElementById("openDownloadFolderBtn");
  const sidebarOpenDirBtn = document.getElementById("openDownloadsBtn");
  const copyAddressBtn = document.getElementById("settingsCopyAddressBtn");

  if (openBtn) {
    openBtn.addEventListener("click", () => {
      openSettingsView();
    });
  }

  if (copyAddressBtn) {
    copyAddressBtn.addEventListener("click", async () => {
      const addrEl = document.getElementById("myDeviceAddress");
      const addr = addrEl ? addrEl.textContent.trim() : "";
      if (!addr || addr === "—") {
        showToast("Local address not detected yet", "error");
        return;
      }
      try {
        await navigator.clipboard.writeText(addr);
        showToast(`Address ${addr} copied to clipboard`, "success");
      } catch (_err) {
        showToast("Failed to copy address", "error");
      }
    });
  }

  // Save Device Name Handler
  async function handleSaveDeviceName() {
    if (!nameInput) return;
    const trimmed = nameInput.value.trim();

    if (!trimmed) {
      showToast("Device name cannot be empty", "error");
      return;
    }

    if (trimmed.length > 32) {
      showToast("Device name must be 32 characters or less", "error");
      return;
    }

    try {
      const updated = await setDeviceName(trimmed);
      const myDeviceBadge = document.getElementById("myDeviceName");
      if (myDeviceBadge) {
        myDeviceBadge.textContent = updated.device_name || trimmed;
      }
      nameInput.value = updated.device_name || trimmed;
      nameInput.dataset.saved = nameInput.value;
      syncSaveNameVisibility();
      showToast("Device name updated!", "success");
    } catch (err) {
      console.error("Failed to update device name:", err);
      showToast(`Error updating device name: ${err}`, "error");
    }
  }

  if (saveNameBtn) {
    saveNameBtn.addEventListener("click", handleSaveDeviceName);
  }

  if (nameInput) {
    nameInput.addEventListener("input", syncSaveNameVisibility);
    nameInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSaveDeviceName();
      } else if (e.key === "Escape") {
        nameInput.value = nameInput.dataset.saved || "";
        syncSaveNameVisibility();
      }
    });
  }

  // Change Download Directory Handler
  if (changeDirBtn) {
    changeDirBtn.addEventListener("click", async () => {
      try {
        const picked = await pickFolder();
        // pickFolder returns an array of files or selected path depending on OS dialog
        let selectedPath = null;
        if (typeof picked === "string" && picked.trim().length > 0) {
          selectedPath = picked.trim();
        } else if (Array.isArray(picked) && picked.length > 0) {
          selectedPath = picked[0].path || picked[0];
        }

        if (selectedPath) {
          const newPath = await setDownloadDir(selectedPath);
          const dirDisplay = document.getElementById("settingsDownloadDirDisplay");
          if (dirDisplay) {
            dirDisplay.textContent = newPath;
            dirDisplay.title = newPath;
          }
          showToast("Download directory updated!", "success");
        }
      } catch (err) {
        console.error("Failed to set download directory:", err);
        showToast(`Failed to update download folder: ${err}`, "error");
      }
    });
  }

  // Open Downloads Folder Handlers
  async function handleOpenDownloads() {
    try {
      await openDownloadDir();
    } catch (err) {
      console.error("Failed to open download directory:", err);
      showToast(`Could not open download directory: ${err}`, "error");
    }
  }

  if (openDirBtn) {
    openDirBtn.addEventListener("click", handleOpenDownloads);
  }

  if (sidebarOpenDirBtn) {
    sidebarOpenDirBtn.addEventListener("click", handleOpenDownloads);
  }

  // Speed Limit Selection Handler
  const speedSelect = document.getElementById("settingsSpeedLimitSelect");
  if (speedSelect) {
    speedSelect.addEventListener("change", async () => {
      try {
        const val = Number(speedSelect.value) || 0;
        await setSpeedLimit(val);
        showToast("Upload speed limit updated", "success");
      } catch (err) {
        console.error("Failed to update speed limit:", err);
        showToast(`Failed to update speed limit: ${err}`, "error");
      }
    });
  }

  // Minimize to Tray Toggle Handler
  const minToggle = document.getElementById("settingsMinimizeToTrayToggle");
  if (minToggle) {
    minToggle.addEventListener("change", async (e) => {
      try {
        const notifToggle = document.getElementById("settingsNotificationsToggle");
        await setTraySettings({
          minimizeToTray: e.target.checked,
          notificationsEnabled: notifToggle ? notifToggle.checked : true,
        });
        showToast(
          e.target.checked
            ? "Closing window will minimize to system tray"
            : "Closing window will exit application",
          "info"
        );
      } catch (err) {
        console.error("Failed to update tray setting:", err);
        showToast(`Failed to update setting: ${err}`, "error");
      }
    });
  }

  // Desktop Notifications Toggle Handler
  const notifToggle = document.getElementById("settingsNotificationsToggle");
  if (notifToggle) {
    notifToggle.addEventListener("change", async (e) => {
      try {
        const minToggle = document.getElementById("settingsMinimizeToTrayToggle");
        await setTraySettings({
          minimizeToTray: minToggle ? minToggle.checked : true,
          notificationsEnabled: e.target.checked,
        });
        showToast(
          e.target.checked
            ? "Desktop notifications enabled"
            : "Desktop notifications disabled",
          "info"
        );
      } catch (err) {
        console.error("Failed to update notification setting:", err);
        showToast(`Failed to update setting: ${err}`, "error");
      }
    });
  }

  // End-to-End Encryption Toggle Handler
  const encToggle = document.getElementById("settingsEncryptionToggle");
  if (encToggle) {
    encToggle.addEventListener("change", async (e) => {
      try {
        const enabled = await setEncryptionSettings(e.target.checked);
        showToast(
          enabled
            ? "End-to-end encryption enabled (X25519 + ChaCha20-Poly1305)"
            : "End-to-end encryption disabled (transfers in plaintext)",
          "info"
        );
      } catch (err) {
        console.error("Failed to update encryption setting:", err);
        showToast(`Failed to update encryption setting: ${err}`, "error");
      }
    });
  }

  // Remember Paired Devices Toggle Handler
  const rememberToggle = document.getElementById("settingsRememberPairedToggle");
  if (rememberToggle) {
    rememberToggle.addEventListener("change", async (e) => {
      try {
        const status = await setRememberPairedDevices(e.target.checked);
        renderPairedDevicesStatus(status);
        showToast(
          status.remember
            ? "Paired devices will be remembered after restart"
            : "Paired devices will be forgotten when the app restarts",
          "info"
        );
      } catch (err) {
        e.target.checked = !e.target.checked;
        console.error("Failed to update remember setting:", err);
        showToast(`Failed to update setting: ${err}`, "error");
      }
    });
  }

  // Forget All Paired Devices Handler
  const forgetPairedBtn = document.getElementById("settingsForgetPairedBtn");
  if (forgetPairedBtn) {
    forgetPairedBtn.addEventListener("click", async () => {
      if (!confirm("Forget all paired devices? You'll need to enter a PIN to pair with them again.")) {
        return;
      }
      try {
        renderPairedDevicesStatus(await forgetPairedDevices());
        showToast("All paired devices forgotten", "info");
      } catch (err) {
        console.error("Failed to forget paired devices:", err);
        showToast(`Failed to forget paired devices: ${err}`, "error");
      }
    });
  }

  // Listen for device info updates
  onDeviceInfoUpdated((info) => {
    if (!info) return;
    const myDeviceBadge = document.getElementById("myDeviceName");
    if (myDeviceBadge && info.device_name) {
      myDeviceBadge.textContent = info.device_name;
    }
    if (nameInput && info.device_name && getCurrentView() !== "settings") {
      nameInput.value = info.device_name;
      nameInput.dataset.saved = info.device_name;
      syncSaveNameVisibility();
    }
  });
}
