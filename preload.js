const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("api", {
  onPeerFound: (callback) => {
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
  sendFile: async (peer, file) => {
    // Convert file to arrayBuffer
    const buffer = await file.arrayBuffer();
    return ipcRenderer.invoke("send-file", peer, {
      name: file.name,
      size: file.size,
      data: buffer, // send raw data instead of relying on path
    });
  },
});
