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
} from "./tauri-bridge.js";
import { showToast } from "./ui.js";

let isSettingsOpen = false;

/**
 * Opens the settings modal and populates current configuration values.
 */
export async function openSettingsModal() {
  const modal = document.getElementById("settings-modal");
  if (!modal) return;

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
  } catch (err) {
    console.error("Failed to load settings data:", err);
  }

  modal.classList.remove("hidden");
  if (modal.setAttribute) {
    modal.setAttribute("aria-hidden", "false");
  }
  isSettingsOpen = true;

  const nameInput = document.getElementById("settingsDeviceNameInput");
  if (nameInput) {
    nameInput.focus();
  }
}

/**
 * Closes the settings modal.
 */
export function closeSettingsModal() {
  const modal = document.getElementById("settings-modal");
  if (!modal) return;

  modal.classList.add("hidden");
  if (modal.setAttribute) {
    modal.setAttribute("aria-hidden", "true");
  }
  isSettingsOpen = false;
}

/**
 * Initializes settings listeners and event handlers.
 */
export function setupSettings() {
  const modal = document.getElementById("settings-modal");
  const openBtn = document.getElementById("openSettingsBtn");
  const closeBtn = document.getElementById("closeSettingsBtn");
  const saveNameBtn = document.getElementById("saveDeviceNameBtn");
  const nameInput = document.getElementById("settingsDeviceNameInput");
  const changeDirBtn = document.getElementById("changeDownloadDirBtn");
  const openDirBtn = document.getElementById("openDownloadFolderBtn");
  const headerOpenDirBtn = document.getElementById("headerOpenDownloadsBtn");

  // Open / Close Modal Handlers
  if (openBtn) {
    openBtn.addEventListener("click", () => {
      openSettingsModal();
    });
  }

  if (closeBtn) {
    closeBtn.addEventListener("click", () => {
      closeSettingsModal();
    });
  }

  if (modal) {
    modal.addEventListener("click", (e) => {
      if (e.target === modal) {
        closeSettingsModal();
      }
    });
  }

  if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && isSettingsOpen) {
        closeSettingsModal();
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

  if (headerOpenDirBtn) {
    headerOpenDirBtn.addEventListener("click", handleOpenDownloads);
  }

  // Listen for device info updates
  onDeviceInfoUpdated((info) => {
    if (!info) return;
    const myDeviceBadge = document.getElementById("myDeviceName");
    if (myDeviceBadge && info.device_name) {
      myDeviceBadge.textContent = info.device_name;
    }
    if (nameInput && info.device_name && !isSettingsOpen) {
      nameInput.value = info.device_name;
    }
  });
}
