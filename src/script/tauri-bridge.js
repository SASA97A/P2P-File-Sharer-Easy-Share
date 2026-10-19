/**
 * Easy Share - Tauri 2.0 IPC Bridge
 * Provides a clean ES module abstraction over Tauri invoke and listen APIs,
 * with graceful browser fallback for standalone testing.
 */

// Helper to resolve the active Tauri invoke method
async function getInvoke() {
  if (typeof window !== "undefined") {
    if (window.__TAURI__?.core?.invoke) {
      return window.__TAURI__.core.invoke;
    }
    if (window.__TAURI_INTERNALS__?.invoke) {
      return window.__TAURI_INTERNALS__.invoke;
    }
    try {
      const core = await import("@tauri-apps/api/core");
      if (core && typeof core.invoke === "function" && window.__TAURI_INTERNALS__) {
        return core.invoke;
      }
    } catch (_e) {
      // Dynamic import unavailable or not in webview
    }
  }
  return null;
}

// Helper to resolve the active Tauri listen method
async function getListen() {
  if (typeof window !== "undefined") {
    if (window.__TAURI__?.event?.listen) {
      return window.__TAURI__.event.listen;
    }
    if (window.__TAURI_INTERNALS__?.listen) {
      return window.__TAURI_INTERNALS__.listen;
    }
    try {
      const event = await import("@tauri-apps/api/event");
      if (event && typeof event.listen === "function" && window.__TAURI_INTERNALS__) {
        return event.listen;
      }
    } catch (_e) {
      // Dynamic import unavailable or not in webview
    }
  }
  return null;
}

// Fallback state for standalone browser execution / testing
const mockState = {
  deviceInfo: {
    device_name: "EasyShare-Browser",
    device_type: "desktop",
    os: "browser",
    version: "2.0.0",
    port: 5050,
    pairing_pin: "839421",
  },
  download_dir: "C:\\Users\\MockUser\\Downloads",
  speed_limit_bps: 0,
  minimize_to_tray: true,
  notifications_enabled: true,
  encryption_enabled: true,
  peers: [],
  history: [
    {
      id: "mock-record-1",
      session_token: "mock-token-1",
      timestamp: Date.now() - 3600000,
      direction: "received",
      peer_name: "Mock-Phone",
      peer_ip: "192.168.1.101",
      status: "completed",
      files: [
        {
          name: "document.pdf",
          path: "C:\\Users\\MockUser\\Downloads\\document.pdf",
          size: 1048576,
        },
      ],
      total_bytes: 1048576,
      error_message: null,
    },
  ],
  listeners: new Map(),
};

function emitMockEvent(eventName, payload) {
  const cbs = mockState.listeners.get(eventName) || [];
  cbs.forEach((cb) => {
    try {
      cb(payload);
    } catch (err) {
      console.error(`[MockEvent] Error in ${eventName} handler:`, err);
    }
  });
}

/**
 * Executes a Tauri IPC command or falls back to mock browser behavior
 */
async function callInvoke(command, args = {}) {
  const invoke = await getInvoke();
  if (invoke) {
    try {
      return await invoke(command, args);
    } catch (invokeErr) {
      console.error(`[TauriBridge] invoke('${command}') failed:`, invokeErr);
      throw invokeErr;
    }
  }

  // Standalone Browser Fallback Implementation
  console.info(`[TauriBridge:Mock] invoke('${command}')`, args);
  switch (command) {
    case "get_my_device_info":
      return { ...mockState.deviceInfo };

    case "get_discovered_peers":
      return [...mockState.peers];

    case "add_manual_peer": {
      const peer = {
        device_name: `Manual-${args.ip}`,
        device_type: "desktop",
        os: "unknown",
        ip: args.ip,
        port: args.port || 5050,
      };
      const existingIdx = mockState.peers.findIndex(
        (p) => `${p.ip}:${p.port}` === `${peer.ip}:${peer.port}`
      );
      if (existingIdx !== -1) {
        mockState.peers[existingIdx] = peer;
      } else {
        mockState.peers.push(peer);
      }
      emitMockEvent("peer-found", peer);
      return peer;
    }

    case "respond_transfer_request": {
      console.log(
        `[Mock] Transfer consent response: ${
          args.accept ? "ACCEPTED" : "DECLINED"
        } for request ${args.requestId || args.request_id}`
      );
      return;
    }

    case "start_transfer": {
      const token = `sess-mock-${Date.now()}`;
      if (args.files && args.files.length > 0) {
        setTimeout(() => {
          let completedCount = 0;
          for (let i = 0; i < args.files.length; i++) {
            const f = args.files[i];
            const fname = f.name || f.path || `file_${i}`;
            emitMockEvent("transfer-progress", {
              session_token: token,
              file_id: `f_${i}`,
              file_name: fname,
              sent_bytes: 500000,
              total_bytes: 1000000,
              percent: 50,
            });
            setTimeout(() => {
              emitMockEvent("transfer-progress", {
                session_token: token,
                file_id: `f_${i}`,
                file_name: fname,
                sent_bytes: 1000000,
                total_bytes: 1000000,
                percent: 100,
              });
              emitMockEvent("transfer-completed", {
                session_token: token,
                file_id: `f_${i}`,
                file_name: fname,
                saved_path: `Downloads/${fname}`,
              });
              completedCount++;
              if (completedCount === args.files.length) {
                emitMockEvent("transfer-batch-completed", {
                  session_token: token,
                  total_files: args.files.length,
                  total_bytes: args.files.length * 1000000,
                });
              }
            }, 600 * (i + 1));
          }
        }, 300);
      }
      return token;
    }

    case "cancel_transfer":
      console.log(
        "[Mock] Transfer cancelled for token:",
        args.sessionToken || args.session_token
      );
      return;

    case "get_download_dir":
      return mockState.download_dir || "Downloads";

    case "set_download_dir": {
      mockState.download_dir = args.path;
      return args.path;
    }

    case "get_speed_limit":
      return mockState.speed_limit_bps || 0;

    case "set_speed_limit": {
      const limit = Number(args.limitBps ?? args.limit_bps ?? 0);
      mockState.speed_limit_bps = limit;
      return limit;
    }

    case "set_device_name": {
      const name = (args.name || "").trim();
      if (!name) throw new Error("Device name cannot be empty");
      mockState.deviceInfo.device_name = name;
      emitMockEvent("device-info-updated", { ...mockState.deviceInfo });
      return { ...mockState.deviceInfo };
    }

    case "open_download_dir":
      console.log("[Mock] Opened download directory");
      return;

    case "get_tray_settings":
      return {
        minimize_to_tray: mockState.minimize_to_tray ?? true,
        minimizeToTray: mockState.minimize_to_tray ?? true,
        notifications_enabled: mockState.notifications_enabled ?? true,
        notificationsEnabled: mockState.notifications_enabled ?? true,
      };

    case "set_tray_settings": {
      const min = args.minimizeToTray ?? args.minimize_to_tray ?? true;
      const notif = args.notificationsEnabled ?? args.notifications_enabled ?? true;
      mockState.minimize_to_tray = min;
      mockState.notifications_enabled = notif;
      return {
        minimize_to_tray: min,
        minimizeToTray: min,
        notifications_enabled: notif,
        notificationsEnabled: notif,
      };
    }

    case "get_encryption_settings":
      return mockState.encryption_enabled ?? true;

    case "set_encryption_settings": {
      const enabled = args.enabled ?? true;
      mockState.encryption_enabled = enabled;
      return enabled;
    }

    case "inspect_paths": {
      const paths = args.paths || [];
      return paths.map((p) => {
        const name = p.split(/[\\/]/).pop() || p;
        return {
          name,
          path: p,
          size: 1024,
        };
      });
    }

    case "pick_files": {
      // In mock mode, if standard file input exists in DOM, click it
      const input = document.getElementById("fileInput");
      if (input) {
        input.click();
      }
      return [];
    }

    case "pick_folder": {
      return [];
    }

    case "get_my_pairing_info": {
      const pin = mockState.deviceInfo.pairing_pin || "839421";
      const formatted_pin = `${pin.slice(0, 3)} - ${pin.slice(3, 6)}`;
      const ip = "127.0.0.1";
      const port = mockState.deviceInfo.port || 5050;
      const direct_url = `easyshare://pair?ip=${ip}&port=${port}&pin=${pin}&name=${encodeURIComponent(mockState.deviceInfo.device_name)}`;
      const qr_svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="160" height="160"><rect width="100" height="100" fill="#ffffff"/><rect x="10" y="10" width="25" height="25" fill="#222222"/><rect x="65" y="10" width="25" height="25" fill="#222222"/><rect x="10" y="65" width="25" height="25" fill="#222222"/><rect x="42" y="42" width="16" height="16" fill="#4caf50"/></svg>`;
      return {
        pin,
        formatted_pin,
        qr_svg,
        direct_url,
        ip,
        port,
      };
    }

    case "regenerate_pairing_pin": {
      const newPin = Math.floor(100000 + Math.random() * 900000).toString();
      mockState.deviceInfo.pairing_pin = newPin;
      const formatted_pin = `${newPin.slice(0, 3)} - ${newPin.slice(3, 6)}`;
      const ip = "127.0.0.1";
      const port = mockState.deviceInfo.port || 5050;
      const direct_url = `easyshare://pair?ip=${ip}&port=${port}&pin=${newPin}&name=${encodeURIComponent(mockState.deviceInfo.device_name)}`;
      const qr_svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="160" height="160"><rect width="100" height="100" fill="#ffffff"/><rect x="10" y="10" width="25" height="25" fill="#222222"/><rect x="65" y="10" width="25" height="25" fill="#222222"/><rect x="10" y="65" width="25" height="25" fill="#222222"/><rect x="42" y="42" width="16" height="16" fill="#4caf50"/></svg>`;
      return {
        pin: newPin,
        formatted_pin,
        qr_svg,
        direct_url,
        ip,
        port,
      };
    }

    case "connect_by_pin": {
      const cleanPin = (args.pin || "").replace(/\D/g, "");
      if (cleanPin.length !== 6) {
        throw new Error(`Invalid 6-digit pairing PIN: '${args.pin}'`);
      }
      const peer = {
        device_name: `Peer-${cleanPin.slice(0, 3)}`,
        device_type: "laptop",
        os: "mock",
        ip: "192.168.1.99",
        port: 5050,
        pairing_pin: cleanPin,
      };
      const existingIdx = mockState.peers.findIndex(
        (p) => `${p.ip}:${p.port}` === `${peer.ip}:${peer.port}`
      );
      if (existingIdx !== -1) {
        mockState.peers[existingIdx] = peer;
      } else {
        mockState.peers.push(peer);
      }
      emitMockEvent("peer-found", peer);
      return peer;
    }

    case "connect_by_address": {
      const addr = (args.address || "").trim();
      if (!addr) {
        throw new Error("Address cannot be empty");
      }
      const peer = {
        device_name: `Peer-${addr.replace(/[^a-zA-Z0-9.-]/g, "_")}`,
        device_type: "desktop",
        os: "unknown",
        ip: addr.includes("://") ? "192.168.1.88" : addr.split(":")[0],
        port: 5050,
      };
      const existingIdx = mockState.peers.findIndex(
        (p) => `${p.ip}:${p.port}` === `${peer.ip}:${peer.port}`
      );
      if (existingIdx !== -1) {
        mockState.peers[existingIdx] = peer;
      } else {
        mockState.peers.push(peer);
      }
      emitMockEvent("peer-found", peer);
      return peer;
    }

    case "get_transfer_history":
      return [...mockState.history];

    case "delete_history_record": {
      const idx = mockState.history.findIndex((r) => r.id === args.record_id);
      if (idx !== -1) {
        mockState.history.splice(idx, 1);
        emitMockEvent("history-updated", [...mockState.history]);
        return true;
      }
      return false;
    }

    case "clear_transfer_history": {
      mockState.history = [];
      emitMockEvent("history-updated", []);
      return null;
    }

    case "show_in_folder":
    case "open_file":
      return null;

    case "pause_transfer":
      emitMockEvent("transfer-paused", { session_token: args.session_token || args.sessionToken });
      return true;

    case "resume_transfer":
      emitMockEvent("transfer-resumed", { session_token: args.session_token || args.sessionToken });
      return true;

    default:
      console.warn(`[TauriBridge] Unhandled mock command: ${command}`);
      return null;
  }
}

/**
 * Subscribes to a Tauri backend event or local mock event bus
 */
async function subscribeEvent(eventName, callback) {
  const listen = await getListen();
  if (listen) {
    return await listen(eventName, (event) => {
      callback(event.payload);
    });
  }

  // Standalone mock listener fallback
  if (!mockState.listeners.has(eventName)) {
    mockState.listeners.set(eventName, []);
  }
  mockState.listeners.get(eventName).push(callback);

  return () => {
    const list = mockState.listeners.get(eventName) || [];
    const idx = list.indexOf(callback);
    if (idx !== -1) {
      list.splice(idx, 1);
    }
  };
}

// -------------------------------------------------------------
// Commands
// -------------------------------------------------------------

/**
 * Returns current device identity and server configuration
 */
export async function getMyDeviceInfo() {
  return await callInvoke("get_my_device_info");
}

/**
 * Returns the currently discovered peer list
 */
export async function getDiscoveredPeers() {
  return await callInvoke("get_discovered_peers");
}

/**
 * Probes and adds a remote peer by IP address and optional port
 */
export async function addManualPeer(ip, port = null) {
  return await callInvoke("add_manual_peer", {
    ip,
    port: port ? Number(port) : null,
  });
}

/**
 * Responds to an interactive transfer consent prompt (Accept / Decline)
 */
export async function respondTransferRequest(requestId, accept) {
  return await callInvoke("respond_transfer_request", {
    requestId,
    request_id: requestId,
    accept: Boolean(accept),
  });
}

/**
 * Initiates an outbound file transfer to a target peer
 */
export async function startTransfer(peer, files) {
  const fileSelections = files.map((f) => ({
    path: f.fullPath || f.path || f.name,
    name: f.name || undefined,
    file_id: f.id || undefined,
    relative_path: f.relativePath || f.relative_path || undefined,
  }));
  return await callInvoke("start_transfer", {
    peer,
    files: fileSelections,
  });
}

/**
 * Cancels an active transfer session
 */
export async function cancelTransfer(sessionToken, peer = null) {
  return await callInvoke("cancel_transfer", {
    sessionToken,
    session_token: sessionToken,
    peer,
  });
}

/**
 * Pauses an active outbound transfer session
 */
export async function pauseTransfer(sessionToken) {
  return await callInvoke("pause_transfer", {
    sessionToken,
    session_token: sessionToken,
  });
}

/**
 * Resumes a paused outbound transfer session
 */
export async function resumeTransfer(sessionToken) {
  return await callInvoke("resume_transfer", {
    sessionToken,
    session_token: sessionToken,
  });
}

/**
 * Returns current download directory path
 */
export async function getDownloadDir() {
  return await callInvoke("get_download_dir");
}

/**
 * Returns the current global outbound speed limit in bytes per second (0 = unlimited)
 */
export async function getSpeedLimit() {
  return await callInvoke("get_speed_limit");
}

/**
 * Updates the global outbound speed limit in bytes per second (0 = unlimited)
 * @param {number} limitBps
 */
export async function setSpeedLimit(limitBps) {
  return await callInvoke("set_speed_limit", {
    limitBps,
    limit_bps: limitBps,
  });
}

/**
 * Opens download directory in the native file manager
 */
export async function openDownloadDir() {
  return await callInvoke("open_download_dir");
}

/**
 * Updates the download directory path
 */
export async function setDownloadDir(path) {
  return await callInvoke("set_download_dir", { path });
}

/**
 * Updates the local device display name
 */
export async function setDeviceName(name) {
  return await callInvoke("set_device_name", { name });
}

/**
 * Retrieves the current system tray and desktop notification settings
 * @returns {Promise<{ minimizeToTray: boolean, notificationsEnabled: boolean }>}
 */
export async function getTraySettings() {
  const res = await callInvoke("get_tray_settings");
  return {
    minimizeToTray: res.minimizeToTray ?? res.minimize_to_tray ?? true,
    notificationsEnabled: res.notificationsEnabled ?? res.notifications_enabled ?? true,
  };
}

/**
 * Updates system tray and desktop notification preferences
 * @param {{ minimizeToTray?: boolean, notificationsEnabled?: boolean }} settings
 * @returns {Promise<{ minimizeToTray: boolean, notificationsEnabled: boolean }>}
 */
export async function setTraySettings(settings) {
  const minimizeToTray = settings.minimizeToTray ?? settings.minimize_to_tray ?? true;
  const notificationsEnabled = settings.notificationsEnabled ?? settings.notifications_enabled ?? true;
  const res = await callInvoke("set_tray_settings", {
    minimizeToTray,
    minimize_to_tray: minimizeToTray,
    notificationsEnabled,
    notifications_enabled: notificationsEnabled,
  });
  return {
    minimizeToTray: res.minimizeToTray ?? res.minimize_to_tray ?? true,
    notificationsEnabled: res.notificationsEnabled ?? res.notifications_enabled ?? true,
  };
}

/**
 * Retrieves whether end-to-end encryption is enabled
 * @returns {Promise<boolean>}
 */
export async function getEncryptionSettings() {
  const res = await callInvoke("get_encryption_settings");
  return typeof res === "boolean" ? res : true;
}

/**
 * Updates the end-to-end encryption preference
 * @param {boolean | { enabled: boolean }} enabled
 * @returns {Promise<boolean>}
 */
export async function setEncryptionSettings(enabled) {
  const isEnabled = typeof enabled === "object" ? !!enabled.enabled : !!enabled;
  const res = await callInvoke("set_encryption_settings", { enabled: isEnabled });
  return typeof res === "boolean" ? res : isEnabled;
}

/**
 * Fetches the local pairing configuration (PIN, QR code SVG, direct URL)
 */
export async function getMyPairingInfo() {
  return await callInvoke("get_my_pairing_info");
}

/**
 * Regenerates the 6-digit numeric pairing PIN and returns the updated pairing payload
 */
export async function regeneratePairingPin() {
  return await callInvoke("regenerate_pairing_pin");
}

/**
 * Connects to a remote peer by scanning the local subnet with its 6-digit PIN
 */
export async function connectByPin(pin) {
  return await callInvoke("connect_by_pin", { pin });
}

/**
 * Connects to a remote peer directly by IP:port or easyshare:// URL
 */
export async function connectByAddress(address) {
  return await callInvoke("connect_by_address", { address });
}

/**
 * Opens the native OS file picker to select one or more files.
 * Returns array of { name, path, size }.
 */
export async function pickFiles() {
  return await callInvoke("pick_files");
}

/**
 * Opens the native OS folder picker to select a directory.
 * Returns array of { name, path, size }.
 */
export async function pickFolder() {
  return await callInvoke("pick_folder");
}

/**
 * Resolves absolute filesystem paths into full file metadata { name, path, size }.
 */
export async function inspectPaths(paths) {
  return await callInvoke("inspect_paths", { paths });
}

// -------------------------------------------------------------
// Event Subscription Helpers
// -------------------------------------------------------------

export async function onDragDrop(cb) {
  return await subscribeEvent("tauri://drag-drop", cb);
}

export async function onDragHover(cb) {
  return await subscribeEvent("tauri://drag-over", cb);
}

export async function onDragLeave(cb) {
  return await subscribeEvent("tauri://drag-leave", cb);
}

export async function onPeerFound(cb) {
  return await subscribeEvent("peer-found", cb);
}

export async function onPeerLost(cb) {
  return await subscribeEvent("peer-lost", cb);
}

export async function onPeerUpdated(cb) {
  return await subscribeEvent("peer-updated", cb);
}

export async function onDeviceInfoUpdated(cb) {
  return await subscribeEvent("device-info-updated", cb);
}

export async function onTransferRequested(cb) {
  return await subscribeEvent("transfer-requested", cb);
}

export async function onTransferProgress(cb) {
  return await subscribeEvent("transfer-progress", cb);
}

export async function onTransferPaused(cb) {
  return await subscribeEvent("transfer-paused", cb);
}

export async function onTransferResumed(cb) {
  return await subscribeEvent("transfer-resumed", cb);
}

export async function onTransferCompleted(cb) {
  return await subscribeEvent("transfer-completed", cb);
}

export async function onTransferBatchCompleted(cb) {
  return await subscribeEvent("transfer-batch-completed", cb);
}

export async function onTransferCancelled(cb) {
  return await subscribeEvent("transfer-cancelled", cb);
}

export async function onReceiveProgress(cb) {
  return await subscribeEvent("receive-progress", cb);
}

export async function onReceiveCompleted(cb) {
  return await subscribeEvent("receive-completed", cb);
}

export async function onReceiveCancelled(cb) {
  return await subscribeEvent("receive-cancelled", cb);
}

export async function onTransferError(cb) {
  return await subscribeEvent("transfer-error", cb);
}

// -------------------------------------------------------------
// History Commands & Events
// -------------------------------------------------------------

/**
 * Returns the full list of transfer history records.
 */
export async function getTransferHistory() {
  return await callInvoke("get_transfer_history");
}

/**
 * Deletes a specific transfer history record by ID.
 */
export async function deleteHistoryRecord(recordId) {
  return await callInvoke("delete_history_record", { record_id: recordId });
}

/**
 * Clears all transfer history records.
 */
export async function clearTransferHistory() {
  return await callInvoke("clear_transfer_history");
}

/**
 * Shows and selects the specified file in native OS file explorer.
 */
export async function showInFolder(path) {
  return await callInvoke("show_in_folder", { path });
}

/**
 * Opens the specified file using the system's default application.
 */
export async function openFile(path) {
  return await callInvoke("open_file", { path });
}

/**
 * Subscribes to history update events.
 */
export async function onHistoryUpdated(cb) {
  return await subscribeEvent("history-updated", cb);
}
