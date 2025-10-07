// Import UI update function
import { updateFileList } from "./ui.js";

// Array to store selected files for sending
let selectedFiles = [];

/**
 * Returns the currently selected files
 * @returns {Array} Array of File objects
 */
export function getFiles() {
  return selectedFiles;
}

/**
 * Adds files to the selected files list and updates UI
 * @param {FileList} files - Files selected by the user
 */
export function addFiles(files) {
  for (const file of files) {
    selectedFiles.push(file);
  }
  updateFileList(selectedFiles, removeFile);
}

/**
 * Removes a file from the selected files list and updates UI
 * @param {number} index - Index of file to remove
 */
function removeFile(index) {
  selectedFiles.splice(index, 1);
  updateFileList(selectedFiles, removeFile);
}
