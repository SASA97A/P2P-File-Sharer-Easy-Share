// Import UI update function
import { updateFileList } from "./ui.js";
import { updateFilesTotalSize } from "./ui.js";

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
    // **KEY FIX:** Ensure the fullPath property is saved for later streaming
    selectedFiles.push({
      name: file.name,
      size: file.size,
      // Electron File objects from input/drop events have a path property
      fullPath: file.path,
      // Add other necessary properties
    });
  }
  updateFileList(selectedFiles, removeFile);
  updateFilesTotalSize(selectedFiles);
}

/**
 * Removes a file from the selected files list and updates UI
 * @param {number} index - Index of file to remove
 */
function removeFile(index) {
  selectedFiles.splice(index, 1);
  updateFileList(selectedFiles, removeFile);
  updateFilesTotalSize(selectedFiles);
}
