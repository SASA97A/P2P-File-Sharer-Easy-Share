import test from "node:test";
import assert from "node:assert/strict";

// Setup minimal document global if needed
if (typeof globalThis.document === "undefined") {
  globalThis.document = {
    getElementById: () => null,
    querySelector: () => null,
    querySelectorAll: () => [],
    createElement: () => ({
      dataset: {},
      style: { setProperty() {} },
      addEventListener() {},
      appendChild() {},
    }),
  };
}

import { addFiles, getFiles, removeFile, clearFiles } from "../src/script/files.js";
import { updateFileList } from "../src/script/ui.js";

test("files module - unique ID assignment and removal by ID", async (t) => {
  clearFiles();

  await t.test("assigns unique id to each added file", () => {
    clearFiles();
    addFiles([
      { fullPath: "C:/folder1/report.pdf", name: "report.pdf", size: 1024 },
      { fullPath: "C:/folder2/report.pdf", name: "report.pdf", size: 2048 },
    ]);

    const files = getFiles();
    assert.equal(files.length, 2, "Both files with identical name from different paths should be added");
    assert.ok(files[0].id, "First file must have a unique id");
    assert.ok(files[1].id, "Second file must have a unique id");
    assert.notEqual(files[0].id, files[1].id, "File IDs must be distinct");
  });

  await t.test("removeFile by unique ID removes exact file without affecting identically named file", () => {
    clearFiles();
    addFiles([
      { fullPath: "C:/folder1/data.csv", name: "data.csv", size: 100 },
      { fullPath: "C:/folder2/data.csv", name: "data.csv", size: 200 },
    ]);

    const initialFiles = getFiles();
    const idToRemove = initialFiles[0].id;
    const remainingId = initialFiles[1].id;

    removeFile(idToRemove);

    const afterRemove = getFiles();
    assert.equal(afterRemove.length, 1);
    assert.equal(afterRemove[0].id, remainingId);
    assert.equal(afterRemove[0].fullPath, "C:/folder2/data.csv");
  });

  await t.test("removeFile by numeric index still works", () => {
    clearFiles();
    addFiles([
      { fullPath: "C:/folder1/a.txt", name: "a.txt", size: 10 },
      { fullPath: "C:/folder2/b.txt", name: "b.txt", size: 20 },
    ]);

    removeFile(0);

    const files = getFiles();
    assert.equal(files.length, 1);
    assert.equal(files[0].name, "b.txt");
  });

  await t.test("updateFileList sets data-file-id and binds remove by fileId", () => {
    const container = {
      innerHTML: "",
      children: [],
      appendChild(child) {
        this.children.push(child);
      },
      querySelectorAll(selector) {
        if (selector === ".remove-btn") {
          return this.children.flatMap((c) => c.buttons || []);
        }
        return [];
      },
    };

    let removedId = null;
    const mockFiles = [
      { id: "test-id-123", name: "doc.pdf", size: 1024 },
    ];

    const origGetElementById = globalThis.document.getElementById;
    const origCreateElement = globalThis.document.createElement;

    globalThis.document.getElementById = (id) => (id === "fileList" ? container : null);
    globalThis.document.createElement = (tag) => {
      const el = {
        tagName: tag,
        dataset: {},
        style: { setProperty() {} },
        listeners: {},
        addEventListener(type, cb) {
          this.listeners[type] = cb;
        },
        buttons: [],
      };
      if (tag === "div") {
        el.className = "";
        let _innerHtml = "";
        Object.defineProperty(el, "innerHTML", {
          set(val) {
            _innerHtml = val;
            const btn = {
              dataset: { fileId: el.dataset.fileId, index: "0" },
              listeners: {},
              addEventListener(type, cb) {
                this.listeners[type] = cb;
              },
            };
            el.buttons.push(btn);
          },
          get() {
            return _innerHtml;
          },
        });
      }
      return el;
    };

    try {
      updateFileList(mockFiles, (id) => {
        removedId = id;
      });

      assert.equal(container.children.length, 1);
      assert.equal(container.children[0].dataset.fileId, "test-id-123");
      assert.equal(container.children[0].id, "file-test-id-123");

      const btn = container.querySelectorAll(".remove-btn")[0];
      btn.listeners.click({ stopPropagation() {}, currentTarget: btn });
      assert.equal(removedId, "test-id-123");
    } finally {
      globalThis.document.getElementById = origGetElementById;
      globalThis.document.createElement = origCreateElement;
    }
  });

  await t.test("addFiles retains relativePath property", () => {
    clearFiles();
    addFiles([
      {
        fullPath: "C:/projects/app/src/index.js",
        name: "index.js",
        size: 512,
        relativePath: "app/src/index.js",
      },
    ]);

    const staged = getFiles();
    assert.equal(staged.length, 1);
    assert.equal(staged[0].relativePath, "app/src/index.js");
  });

  await t.test("updateFileList displays relativePath in file-detail", () => {
    const container = {
      innerHTML: "",
      children: [],
      appendChild(child) {
        this.children.push(child);
      },
      querySelectorAll() {
        return [];
      },
    };

    const mockFiles = [
      {
        id: "rel-id-1",
        name: "main.rs",
        size: 1024,
        relativePath: "Project/src/main.rs",
      },
    ];

    const origGetElementById = globalThis.document.getElementById;
    const origCreateElement = globalThis.document.createElement;

    let renderedHtml = "";
    globalThis.document.getElementById = (id) => (id === "fileList" ? container : null);
    globalThis.document.createElement = (tag) => {
      const el = {
        tagName: tag,
        dataset: {},
        style: { setProperty() {} },
        addEventListener() {},
      };
      if (tag === "div") {
        Object.defineProperty(el, "innerHTML", {
          set(val) {
            renderedHtml = val;
          },
          get() {
            return renderedHtml;
          },
        });
      }
      return el;
    };

    try {
      updateFileList(mockFiles, () => {});
      assert.ok(
        renderedHtml.includes("Project/src/main.rs"),
        `rendered html must contain relative path: ${renderedHtml}`
      );
    } finally {
      globalThis.document.getElementById = origGetElementById;
      globalThis.document.createElement = origCreateElement;
    }
  });
});
