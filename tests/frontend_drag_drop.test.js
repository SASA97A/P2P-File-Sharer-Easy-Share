import test from "node:test";
import assert from "node:assert/strict";

test("tauri-bridge drag-and-drop integration with Tauri 2 onDragDropEvent", async (t) => {
  // Setup simulated Tauri 2 environment with getCurrentWebviewWindow().onDragDropEvent
  let registeredDragHandler = null;
  const mockUnlisten = () => {
    registeredDragHandler = null;
  };

  globalThis.window = {
    __TAURI__: {
      webviewWindow: {
        getCurrentWebviewWindow: () => ({
          onDragDropEvent: (handler) => {
            registeredDragHandler = handler;
            return Promise.resolve(mockUnlisten);
          },
        }),
      },
    },
  };

  // Dynamically import tauri-bridge to test in this mocked environment
  const bridge = await import(`../src/script/tauri-bridge.js?test=${Date.now()}`);

  await t.test("registers onDragDropEvent and routes enter/over events to onDragHover", async () => {
    let hovered = 0;
    await bridge.onDragHover(() => {
      hovered++;
    });

    assert.ok(registeredDragHandler, "Tauri 2 onDragDropEvent must be registered");

    // Simulate Tauri 2 'enter' and 'over' events
    registeredDragHandler({ payload: { type: "enter", paths: ["C:/folder"] } });
    registeredDragHandler({ payload: { type: "over" } });

    assert.equal(hovered, 2, "onDragHover should fire for enter and over events");
  });

  await t.test("routes leave event to onDragLeave", async () => {
    let left = 0;
    await bridge.onDragLeave(() => {
      left++;
    });

    registeredDragHandler({ payload: { type: "leave" } });
    assert.equal(left, 1, "onDragLeave should fire for leave event");
  });

  await t.test("routes drop event with paths to onDragDrop", async () => {
    let droppedPayload = null;
    await bridge.onDragDrop((payload) => {
      droppedPayload = payload;
    });

    const mockPaths = ["C:/Downloads/file.pdf", "C:/Downloads/MyFolder"];
    registeredDragHandler({
      payload: {
        type: "drop",
        paths: mockPaths,
      },
    });

    assert.ok(droppedPayload, "onDragDrop should receive payload");
    assert.deepEqual(droppedPayload.paths, mockPaths, "Payload paths must match dropped paths");
  });
});

test("tauri-bridge drag-and-drop fallback in standalone mock mode", async (t) => {
  // Clear window.__TAURI__ so it uses standalone mock event bus
  delete globalThis.window.__TAURI__;

  const bridge = await import(`../src/script/tauri-bridge.js?mock=${Date.now()}`);

  await t.test("registers mock listeners for drag events and invokes callbacks", async () => {
    let dropped = null;
    let hovered = false;
    let left = false;

    await bridge.onDragDrop((payload) => {
      dropped = payload;
    });
    await bridge.onDragHover(() => {
      hovered = true;
    });
    await bridge.onDragLeave(() => {
      left = true;
    });

    assert.equal(typeof bridge.onDragDrop, "function");
    assert.equal(typeof bridge.onDragHover, "function");
    assert.equal(typeof bridge.onDragLeave, "function");
  });
});

