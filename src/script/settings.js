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
  } catch (err) {
    console.error("Failed to load settings data:", err);
  }
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
    nameInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSaveDeviceName();
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

  // Listen for device info updates
  onDeviceInfoUpdated((info) => {
    if (!info) return;
    const myDeviceBadge = document.getElementById("myDeviceName");
    if (myDeviceBadge && info.device_name) {
      myDeviceBadge.textContent = info.device_name;
    }
    if (nameInput && info.device_name && getCurrentView() !== "settings") {
      nameInput.value = info.device_name;
    }
  });
}
