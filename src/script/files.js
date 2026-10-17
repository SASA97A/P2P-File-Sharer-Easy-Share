/**
 * Easy Share - Files Module
 * Handles file staging, drag-and-drop ingestion, removal, and sizing
 */

import { updateFileList, updateFilesTotalSize } from "./ui.js";

// Staged files list
let selectedFiles = [];

/**
 * Generates a unique identifier for a staged file
 * @returns {string}
 */
export function generateFileId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `f_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * Returns current staged files array
 * @returns {Array<{id: string, name: string, size: number, fullPath: string, path: string}>}
 */
export function getFiles() {
  return [...selectedFiles];
}

/**
 * Clears all staged files and updates UI
 */
export function clearFiles() {
  selectedFiles = [];
  updateFileList(selectedFiles, removeFile);
  updateFilesTotalSize(selectedFiles);
}

/**
 * Adds files from input dialog, drag-drop, or path objects to the staging list
 * @param {FileList|Array<File|Object|string>} files
 */
export function addFiles(files) {
  if (!files) return;

  const fileArray = Array.from(files);
  for (const item of fileArray) {
    let fileObj = null;

    if (typeof item === "string") {
      const fileName = item.split(/[\\/]/).pop() || item;
      fileObj = {
        id: generateFileId(),
        name: fileName,
        size: 0,
        fullPath: item,
        path: item,
      };
    } else if (item && typeof item === "object") {
      const name = item.name || (item.fullPath || item.path || "unnamed_file").split(/[\\/]/).pop();
      const size = Number(item.size) || 0;
      const fullPath = item.path || item.fullPath || item.filePath || name;
      const id = item.id || item.file_id || generateFileId();

      fileObj = {
        id,
        name,
        size,
        fullPath,
        path: fullPath,
      };
    }

    if (fileObj) {
      // Avoid exact duplicate path insertions
      const duplicate = selectedFiles.some(
        (existing) =>
          existing.fullPath === fileObj.fullPath &&
          existing.name === fileObj.name &&
          existing.size === fileObj.size
      );

      if (!duplicate) {
        selectedFiles.push(fileObj);
      }
    }
  }

  updateFileList(selectedFiles, removeFile);
  updateFilesTotalSize(selectedFiles);
}

/**
 * Removes a file by its unique file ID or index and updates UI
 * @param {number|string} idOrIndex
 */
export function removeFile(idOrIndex) {
  let index = -1;
  if (typeof idOrIndex === "string" && isNaN(Number(idOrIndex))) {
    index = selectedFiles.findIndex((f) => f.id === idOrIndex);
  } else {
    const numIndex = Number(idOrIndex);
    if (!isNaN(numIndex) && numIndex >= 0 && numIndex < selectedFiles.length) {
      index = numIndex;
    } else if (typeof idOrIndex === "string") {
      index = selectedFiles.findIndex((f) => f.id === idOrIndex);
    }
  }

  if (index >= 0 && index < selectedFiles.length) {
    selectedFiles.splice(index, 1);
    updateFileList(selectedFiles, removeFile);
    updateFilesTotalSize(selectedFiles);
  }
}

/**
 * Formats byte size into human-readable string
 * @param {number} bytes
 * @returns {string}
 */
export function formatFileSize(bytes) {
  const b = Number(bytes) || 0;
  if (b >= 1024 ** 3) {
    return (b / 1024 ** 3).toFixed(2) + " GB";
  }
  if (b >= 1024 ** 2) {
    return (b / 1024 ** 2).toFixed(2) + " MB";
  }
  if (b >= 1024) {
    return (b / 1024).toFixed(1) + " KB";
  }
  return b + " B";
}
