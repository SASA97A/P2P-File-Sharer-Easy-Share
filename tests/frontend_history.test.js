import test from "node:test";
import assert from "node:assert/strict";

import {
  getTransferHistory,
  deleteHistoryRecord,
  clearTransferHistory,
  showInFolder,
  openFile,
  onHistoryUpdated,
} from "../src/script/tauri-bridge.js";

test("transfer history bridge API", async (t) => {
  await t.test("getTransferHistory returns array of history records", async () => {
    const records = await getTransferHistory();
    assert.ok(Array.isArray(records));
  });

  await t.test("deleteHistoryRecord removes a record by id", async () => {
    const deleted = await deleteHistoryRecord("mock-record-1");
    assert.equal(typeof deleted, "boolean");
  });

  await t.test("clearTransferHistory removes all records", async () => {
    await assert.doesNotReject(async () => {
      await clearTransferHistory();
    });
    const records = await getTransferHistory();
    assert.equal(records.length, 0);
  });

  await t.test("showInFolder invokes without throwing error", async () => {
    await assert.doesNotReject(async () => {
      await showInFolder("C:\\Downloads\\document.pdf");
    });
  });

  await t.test("openFile invokes without throwing error", async () => {
    await assert.doesNotReject(async () => {
      await openFile("C:\\Downloads\\document.pdf");
    });
  });

  await t.test("onHistoryUpdated registers listener without throwing", async () => {
    let called = false;
    await onHistoryUpdated((records) => {
      called = true;
    });
    assert.equal(typeof onHistoryUpdated, "function");
  });
});
