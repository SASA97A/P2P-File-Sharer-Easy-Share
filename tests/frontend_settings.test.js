import test from "node:test";
import assert from "node:assert/strict";

import {
  getDownloadDir,
  setDownloadDir,
  setDeviceName,
  openDownloadDir,
  onDeviceInfoUpdated,
  getMyDeviceInfo,
  getSpeedLimit,
  setSpeedLimit,
  getTraySettings,
  setTraySettings,
} from "../src/script/tauri-bridge.js";

test("settings bridge API", async (t) => {
  await t.test("getSpeedLimit returns default 0 (unlimited)", async () => {
    const limit = await getSpeedLimit();
    assert.equal(typeof limit, "number");
    assert.equal(limit, 0);
  });

  await t.test("setSpeedLimit updates speed limit", async () => {
    const res = await setSpeedLimit(5242880);
    assert.equal(res, 5242880);

    const current = await getSpeedLimit();
    assert.equal(current, 5242880);
  });

  await t.test("getDownloadDir returns current download folder path", async () => {
    const dir = await getDownloadDir();
    assert.equal(typeof dir, "string");
    assert.ok(dir.length > 0);
  });

  await t.test("setDownloadDir invokes set_download_dir with new path", async () => {
    const newPath = "C:\\Custom\\Downloads";
    const result = await setDownloadDir(newPath);
    assert.equal(result, newPath);

    const current = await getDownloadDir();
    assert.equal(current, newPath);
  });

  await t.test("setDeviceName invokes set_device_name and updates device info", async () => {
    const newName = "Safar-Workstation";
    const updated = await setDeviceName(newName);
    assert.equal(updated.device_name, newName);

    const devInfo = await getMyDeviceInfo();
    assert.equal(devInfo.device_name, newName);
  });

  await t.test("openDownloadDir invokes open_download_dir without error", async () => {
    await assert.doesNotReject(async () => {
      await openDownloadDir();
    });
  });

  await t.test("onDeviceInfoUpdated registers listener without throwing", async () => {
    let triggered = false;
    await onDeviceInfoUpdated((info) => {
      triggered = true;
    });
    assert.equal(typeof onDeviceInfoUpdated, "function");
  });

  await t.test("getTraySettings returns default true for minimizeToTray and notificationsEnabled", async () => {
    const settings = await getTraySettings();
    assert.equal(typeof settings.minimizeToTray, "boolean");
    assert.equal(typeof settings.notificationsEnabled, "boolean");
    assert.equal(settings.minimizeToTray, true);
    assert.equal(settings.notificationsEnabled, true);
  });

  await t.test("setTraySettings updates and persists minimizeToTray and notificationsEnabled", async () => {
    const updated = await setTraySettings({
      minimizeToTray: false,
      notificationsEnabled: false,
    });
    assert.equal(updated.minimizeToTray, false);
    assert.equal(updated.notificationsEnabled, false);

    const current = await getTraySettings();
    assert.equal(current.minimizeToTray, false);
    assert.equal(current.notificationsEnabled, false);
  });

  await t.test("getEncryptionSettings returns default enabled (true)", async () => {
    const { getEncryptionSettings } = await import("../src/script/tauri-bridge.js");
    const enabled = await getEncryptionSettings();
    assert.equal(typeof enabled, "boolean");
    assert.equal(enabled, true);
  });

  await t.test("setEncryptionSettings updates and persists encryption setting", async () => {
    const { getEncryptionSettings, setEncryptionSettings } = await import("../src/script/tauri-bridge.js");
    const res = await setEncryptionSettings(false);
    assert.equal(res, false);

    const current = await getEncryptionSettings();
    assert.equal(current, false);

    // Reset back to true
    await setEncryptionSettings(true);
    assert.equal(await getEncryptionSettings(), true);
  });
});
