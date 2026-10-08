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
  peers: [],
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
            }, 600);
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
      return "Downloads";

    case "open_download_dir":
      console.log("[Mock] Opened download directory");
      return;

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
 * Returns current download directory path
 */
export async function getDownloadDir() {
  return await callInvoke("get_download_dir");
}

/**
 * Opens download directory in the native file manager
 */
export async function openDownloadDir() {
  return await callInvoke("open_download_dir");
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

// -------------------------------------------------------------
// Event Subscription Helpers
// -------------------------------------------------------------

export async function onPeerFound(cb) {
  return await subscribeEvent("peer-found", cb);
}

export async function onPeerLost(cb) {
  return await subscribeEvent("peer-lost", cb);
}

export async function onPeerUpdated(cb) {
  return await subscribeEvent("peer-updated", cb);
}

export async function onTransferRequested(cb) {
  return await subscribeEvent("transfer-requested", cb);
}

export async function onTransferProgress(cb) {
  return await subscribeEvent("transfer-progress", cb);
}

export async function onTransferCompleted(cb) {
  return await subscribeEvent("transfer-completed", cb);
}

export async function onTransferError(cb) {
  return await subscribeEvent("transfer-error", cb);
}
