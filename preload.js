const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("api", {
  onPeerFound: (callback) => {
    // Remove previous listeners to avoid duplicates
    ipcRenderer.removeAllListeners("peer-found");
    ipcRenderer.on("peer-found", (event, peer) => callback(peer));
  },
  onPeerLost: (callback) =>
    ipcRenderer.on("peer-lost", (event, peer) => callback(peer)),
  onFileReceived: (callback) => {
    ipcRenderer.removeAllListeners("file-received");
    ipcRenderer.on("file-received", (event, path) => callback(path));
  },
  onSendProgress: (callback) => {
    ipcRenderer.removeAllListeners("send-progress");
    ipcRenderer.on("send-progress", (event, progress) => callback(progress));
  },
  sendFile: (peer, fileBuffer) =>
    ipcRenderer.invoke("send-file", peer, fileBuffer),
});
