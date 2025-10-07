// Import Electron modules for IPC communication
const { contextBridge, ipcRenderer } = require("electron");
const { dialog } = require("electron");

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
    // For web File objects, we need to save to a temporary file first
    // and then pass the path to the main process
    const tempPath = await new Promise((resolve) => {
      const reader = new FileReader();
      reader.onload = async () => {
        // Use dialog to get save location
        const { filePath } = await dialog.showSaveDialog({
          title: 'Save Temporary File',
          defaultPath: file.name,
          buttonLabel: 'Save'
        });
        
        if (!filePath) {
          resolve(null);
          return;
        }
        
        resolve(filePath);
      };
      reader.readAsArrayBuffer(file);
    });
    
    if (!tempPath) return Promise.reject("File save cancelled");
    
    return ipcRenderer.invoke("send-file", peer, {
      name: file.name,
      size: file.size,
      path: tempPath
    });
  },
});
