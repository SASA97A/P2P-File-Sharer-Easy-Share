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
  // Function to send a file to a peer
  sendFile: async (peer, file) => {
    // Convert file to arrayBuffer for transfer
    const buffer = await file.arrayBuffer();
    return ipcRenderer.invoke("send-file", peer, {
      name: file.name,
      size: file.size,
      data: buffer, // Send raw data instead of relying on file path
    });
  },
  // Function to choose download location
  chooseDownloadLocation: () => {
    return ipcRenderer.invoke("choose-download-location");
  },
  // Get current download location
  getDownloadLocation: () => {
    return ipcRenderer.invoke("get-download-location");
  },
});
