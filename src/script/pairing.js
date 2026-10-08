/**
 * Easy Share - Device Pairing Modal & IPC Integration
 * Handles PIN display, QR Code rendering, Subnet PIN sweep, and Direct IP pairing
 * with a 3-tab segmented interface (PIN, QR Code, Direct IP).
 */

import {
  getMyPairingInfo,
  regeneratePairingPin,
  connectByPin,
  connectByAddress,
} from "./tauri-bridge.js";
import { upsertPeer, setSelectedPeer } from "./peers.js";
import { showToast } from "./ui.js";

// Active pairing information payload cache
let activePairingInfo = null;

/**
 * Formats a 6-digit numeric string as 'XXX - XXX'
 * @param {string} pin
 * @returns {string}
 */
function formatPinDisplay(pin) {
  if (!pin) return "--- ---";
  const clean = String(pin).replace(/\D/g, "");
  if (clean.length > 3) {
    return `${clean.slice(0, 3)} - ${clean.slice(3, 6)}`;
  }
  return clean;
}

/**
 * Updates the modal UI with current device pairing info
 * @param {Object} info
 */
function renderPairingInfo(info) {
  if (!info) return;
  activePairingInfo = info;

  const pinDisplay = document.getElementById("myPairingPin");
  const qrBox = document.getElementById("myPairingQrBox");
  const directInfo = document.getElementById("myPairingDirectInfo");
  const localAddressDisplay = document.getElementById("myLocalAddressDisplay");

  if (pinDisplay) {
    pinDisplay.textContent = info.formatted_pin || formatPinDisplay(info.pin);
  }

  if (qrBox) {
    if (info.qr_svg) {
      qrBox.innerHTML = info.qr_svg;
    } else {
      qrBox.innerHTML = '<span class="pairing-address-hint">QR Code Unavailable</span>';
    }
  }

  if (directInfo) {
    if (info.ip && info.port) {
      directInfo.textContent = `Direct: ${info.ip}:${info.port}`;
    } else {
      directInfo.textContent = "";
    }
  }

  if (localAddressDisplay) {
    if (info.ip && info.port) {
      localAddressDisplay.textContent = `${info.ip}:${info.port}`;
    } else if (info.ip) {
      localAddressDisplay.textContent = `${info.ip}:5050`;
    } else {
      localAddressDisplay.textContent = "127.0.0.1:5050";
    }
  }
}

/**
 * Displays status message / indicator inside PIN view
 * @param {string} message
 * @param {"loading"|"error"|"success"} type
 */
function setPinStatus(message, type = "loading") {
  const statusBox = document.getElementById("pairingStatusBox");
  const statusMsg = document.getElementById("pairingStatusMessage");

  if (!statusBox || !statusMsg) return;

  statusBox.className = `pairing-status-message-box ${type}`;
  statusMsg.textContent = message;
  statusBox.style.display = "flex";
}

/**
 * Clears PIN status box
 */
function clearPinStatus() {
  const statusBox = document.getElementById("pairingStatusBox");
  const statusMsg = document.getElementById("pairingStatusMessage");

  if (statusBox) {
    statusBox.style.display = "none";
    statusBox.className = "pairing-status-message-box";
  }
  if (statusMsg) {
    statusMsg.textContent = "";
  }
}

/**
 * Displays status message / indicator inside Direct IP view
 * @param {string} message
 * @param {"loading"|"error"|"success"} type
 */
function setAddressStatus(message, type = "loading") {
  const statusBox = document.getElementById("addressStatusBox");
  const statusMsg = document.getElementById("addressStatusMessage");

  if (!statusBox || !statusMsg) return;

  statusBox.className = `pairing-status-message-box ${type}`;
  statusMsg.textContent = message;
  statusBox.style.display = "flex";
}

/**
 * Clears Address status box
 */
function clearAddressStatus() {
  const statusBox = document.getElementById("addressStatusBox");
  const statusMsg = document.getElementById("addressStatusMessage");

  if (statusBox) {
    statusBox.style.display = "none";
    statusBox.className = "pairing-status-message-box";
  }
  if (statusMsg) {
    statusMsg.textContent = "";
  }
}

/**
 * Clears all status indicators across tabs
 */
function clearAllStatus() {
  clearPinStatus();
  clearAddressStatus();
}

/**
 * Initializes pairing dialog event bindings and IPC commands
 */
export function setupPairing() {
  const openBtn = document.getElementById("openPairingBtn");
  const closeBtn = document.getElementById("closePairingBtn");
  const modal = document.getElementById("pairing-modal");

  // Tab buttons
  const tabPinBtn = document.getElementById("tabPinBtn");
  const tabQrBtn = document.getElementById("tabQrBtn");
  const tabAddressBtn = document.getElementById("tabAddressBtn");

  // Tab view panels
  const viewPin = document.getElementById("viewPin");
  const viewQr = document.getElementById("viewQr");
  const viewAddress = document.getElementById("viewAddress");

  const tabs = [
    { btn: tabPinBtn, view: viewPin, id: "pin" },
    { btn: tabQrBtn, view: viewQr, id: "qr" },
    { btn: tabAddressBtn, view: viewAddress, id: "address" },
  ];

  // Actions in Pin View
  const copyPinBtn = document.getElementById("copyPinBtn");
  const refreshPinBtn = document.getElementById("refreshPinBtn");
  const peerPinInput = document.getElementById("peerPinInput");
  const connectPinBtn = document.getElementById("connectPinBtn");

  // Actions in QR View
  const copyLinkBtn = document.getElementById("copyLinkBtn");

  // Actions in Address View
  const peerAddressInput = document.getElementById("peerAddressInput");
  const connectAddressBtn = document.getElementById("connectAddressBtn");
  const copyMyAddressBtn = document.getElementById("copyMyAddressBtn");

  // -------------------------------------------------------------
  // Tab Switching Logic
  // -------------------------------------------------------------
  function switchTab(targetId) {
    clearAllStatus();

    tabs.forEach(({ btn, view, id }) => {
      const isActive = id === targetId;
      if (btn) {
        btn.classList.toggle("active", isActive);
        btn.setAttribute("aria-selected", String(isActive));
        btn.setAttribute("tabindex", isActive ? "0" : "-1");
      }
      if (view) {
        view.classList.toggle("hidden", !isActive);
      }
    });

    if (targetId === "pin" && peerPinInput) {
      setTimeout(() => peerPinInput.focus(), 50);
    } else if (targetId === "address" && peerAddressInput) {
      setTimeout(() => peerAddressInput.focus(), 50);
    }
  }

  tabs.forEach(({ btn, id }, index) => {
    if (!btn) return;

    btn.addEventListener("click", () => switchTab(id));

    btn.addEventListener("keydown", (e) => {
      let nextIndex = index;
      if (e.key === "ArrowRight") {
        nextIndex = (index + 1) % tabs.length;
      } else if (e.key === "ArrowLeft") {
        nextIndex = (index - 1 + tabs.length) % tabs.length;
      }

      if (nextIndex !== index) {
        e.preventDefault();
        tabs[nextIndex].btn?.focus();
        switchTab(tabs[nextIndex].id);
      }
    });
  });

  // -------------------------------------------------------------
  // Modal Open & Close Lifecycle
  // -------------------------------------------------------------
  async function openModal() {
    if (!modal) return;
    clearAllStatus();

    if (peerPinInput) peerPinInput.value = "";
    if (peerAddressInput) peerAddressInput.value = "";

    // Default to PIN tab on opening
    switchTab("pin");

    modal.classList.remove("hidden");
    modal.setAttribute("aria-hidden", "false");

    try {
      const info = await getMyPairingInfo();
      renderPairingInfo(info);
    } catch (err) {
      console.error("Failed to load pairing info:", err);
      setPinStatus("Failed to retrieve local pairing information", "error");
    }
  }

  function closeModal() {
    if (!modal) return;
    modal.classList.add("hidden");
    modal.setAttribute("aria-hidden", "true");
    clearAllStatus();
  }

  if (openBtn) {
    openBtn.addEventListener("click", openModal);
  }

  if (closeBtn) {
    closeBtn.addEventListener("click", closeModal);
  }

  if (modal) {
    modal.addEventListener("click", (e) => {
      if (e.target === modal) {
        closeModal();
      }
    });
  }

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && modal && !modal.classList.contains("hidden")) {
      closeModal();
    }
  });

  // -------------------------------------------------------------
  // Copy Actions (PIN, Direct Link, My Address)
  // -------------------------------------------------------------
  if (copyPinBtn) {
    copyPinBtn.addEventListener("click", async () => {
      const pin = activePairingInfo?.pin || activePairingInfo?.formatted_pin;
      if (!pin) {
        showToast("No pairing PIN available", "error");
        return;
      }
      try {
        await navigator.clipboard.writeText(String(pin).replace(/\D/g, ""));
        showToast("Pairing PIN copied to clipboard", "success");
      } catch (_err) {
        showToast("Failed to copy PIN to clipboard", "error");
      }
    });
  }

  if (copyLinkBtn) {
    copyLinkBtn.addEventListener("click", async () => {
      const link = activePairingInfo?.direct_url;
      if (!link) {
        showToast("No pairing URL available", "error");
        return;
      }
      try {
        await navigator.clipboard.writeText(link);
        showToast("Pairing link copied to clipboard", "success");
      } catch (_err) {
        showToast("Failed to copy link to clipboard", "error");
      }
    });
  }

  if (copyMyAddressBtn) {
    copyMyAddressBtn.addEventListener("click", async () => {
      const ip = activePairingInfo?.ip || "127.0.0.1";
      const port = activePairingInfo?.port || 5050;
      const addr = `${ip}:${port}`;
      try {
        await navigator.clipboard.writeText(addr);
        showToast(`Address ${addr} copied to clipboard`, "success");
      } catch (_err) {
        showToast("Failed to copy address", "error");
      }
    });
  }

  // -------------------------------------------------------------
  // Regenerate PIN Action
  // -------------------------------------------------------------
  if (refreshPinBtn) {
    refreshPinBtn.addEventListener("click", async () => {
      refreshPinBtn.disabled = true;
      try {
        const newInfo = await regeneratePairingPin();
        renderPairingInfo(newInfo);
        showToast("Generated new pairing PIN and QR Code", "info");
      } catch (err) {
        console.error("Failed to regenerate PIN:", err);
        showToast("Failed to regenerate pairing PIN", "error");
      } finally {
        refreshPinBtn.disabled = false;
      }
    });
  }

  // -------------------------------------------------------------
  // PIN Input Auto-Formatting (XXX - XXX)
  // -------------------------------------------------------------
  if (peerPinInput) {
    peerPinInput.addEventListener("input", (e) => {
      const digits = e.target.value.replace(/\D/g, "").slice(0, 6);
      if (digits.length > 3) {
        e.target.value = `${digits.slice(0, 3)} - ${digits.slice(3)}`;
      } else {
        e.target.value = digits;
      }
    });

    peerPinInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleConnectByPin();
      }
    });
  }

  // -------------------------------------------------------------
  // Connect by PIN Action
  // -------------------------------------------------------------
  async function handleConnectByPin() {
    if (!peerPinInput) return;
    const cleanPin = peerPinInput.value.replace(/\D/g, "");

    if (cleanPin.length !== 6) {
      setPinStatus("Please enter a valid 6-digit numeric PIN", "error");
      return;
    }

    setPinStatus("Scanning local subnet for device with matching PIN...", "loading");
    if (connectPinBtn) connectPinBtn.disabled = true;

    try {
      const peer = await connectByPin(cleanPin);
      upsertPeer(peer);
      setSelectedPeer(peer);
      showToast(`Connected to ${peer.device_name || "device"}!`, "success");
      closeModal();
    } catch (err) {
      console.error("Connect by PIN failed:", err);
      const msg = typeof err === "string" ? err : err.message || "Device not found on local network";
      setPinStatus(msg, "error");
      showToast(msg, "error");
    } finally {
      if (connectPinBtn) connectPinBtn.disabled = false;
    }
  }

  if (connectPinBtn) {
    connectPinBtn.addEventListener("click", handleConnectByPin);
  }

  // -------------------------------------------------------------
  // Connect by Address Action
  // -------------------------------------------------------------
  async function handleConnectByAddress() {
    if (!peerAddressInput) return;
    const address = peerAddressInput.value.trim();

    if (!address) {
      setAddressStatus("Please enter an IP:port address or easyshare:// link", "error");
      return;
    }

    setAddressStatus("Connecting to remote peer address...", "loading");
    if (connectAddressBtn) connectAddressBtn.disabled = true;

    try {
      const peer = await connectByAddress(address);
      upsertPeer(peer);
      setSelectedPeer(peer);
      showToast(`Connected to ${peer.device_name || "device"}!`, "success");
      closeModal();
    } catch (err) {
      console.error("Connect by address failed:", err);
      const msg = typeof err === "string" ? err : err.message || "Failed to reach remote device";
      setAddressStatus(msg, "error");
      showToast(msg, "error");
    } finally {
      if (connectAddressBtn) connectAddressBtn.disabled = false;
    }
  }

  if (connectAddressBtn) {
    connectAddressBtn.addEventListener("click", handleConnectByAddress);
  }

  if (peerAddressInput) {
    peerAddressInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleConnectByAddress();
      }
    });
  }
}
