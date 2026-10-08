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
    room_id: null,
    port: 5050,
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
        room_id: null,
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

    case "set_room_id": {
      mockState.deviceInfo.room_id = args.room ? args.room.trim() : null;
      return { ...mockState.deviceInfo };
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
 * Updates the local room ID filter tag and restarts discovery
 */
export async function setRoomId(room) {
  const normalized = room && room.trim().length > 0 ? room.trim() : null;
  return await callInvoke("set_room_id", { room: normalized });
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
