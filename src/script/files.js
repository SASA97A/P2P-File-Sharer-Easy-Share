import { updateFileList } from "./ui.js";

let selectedFiles = [];

export function getFiles() {
  return selectedFiles;
}

export function addFiles(files) {
  for (const file of files) {
    selectedFiles.push(file);
  }
  updateFileList(selectedFiles, removeFile);
}

function removeFile(index) {
  selectedFiles.splice(index, 1);
  updateFileList(selectedFiles, removeFile);
}
