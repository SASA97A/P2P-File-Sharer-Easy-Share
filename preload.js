// Import Electron modules for IPC communication
const { contextBridge, ipcRenderer } = require("electron");

// Expose a limited API to the renderer process
contextBridge.exposeInMainWorld("api", {
  // Register callback for peer discovery events
  onPeerFound: (callback) => {
    ipcRenderer.removeAllListeners("peer-found");
    ipcRenderer.on("peer-found", (event, peer) => callback(peer));
  },
  // Register callback for peer lost events
  onPeerLost: (callback) =>
    ipcRenderer.on("peer-lost", (event, peer) => callback(peer)),
  // Register callback for file received events
  onFileReceived: (callback) => {
    ipcRenderer.removeAllListeners("file-received");
    ipcRenderer.on("file-received", (event, path) => callback(path));
  },
  // Register callback for send progress events
  onSendProgress: (callback) => {
    ipcRenderer.removeAllListeners("send-progress");
    ipcRenderer.on("send-progress", (event, progress) => callback(progress));
  },

  // 🎯 FIX A: New function to trigger file selection in the Main Process
  openFileSelectDialog: () => {
    return ipcRenderer.invoke("open-file-select-dialog");
  },

  // 🎯 FIX B: Function to send a file to a peer - Pass the structured object
  sendFile: (peer, fileToSend) => {
    // fileToSend now contains name, size, and the verified fullPath (see files.js)
    return ipcRenderer.invoke("send-file", peer, fileToSend);
  },
});