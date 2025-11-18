// Import required Electron modules
const { app, ipcMain, dialog } = require("electron");
const os = require("os");
const fs = require("fs");
const path = require("path");
// Import custom modules for window creation and network services
const { createWindow } = require("./core/window");
const { startBonjour } = require("./core/bonjour");
const { startUdpDiscovery } = require("./core/udp");
const { startTcpServer } = require("./core/tcp");
const { setupFileSender } = require("./core/fileSender");

// Configuration constants
const serviceName = os.hostname(); // Use computer name as service name
const SERVICE_TYPE = "myfileshare"; // Service type for discovery
const PORT = 5050; // TCP port for file transfer
const UDP_PORT = 41234; // UDP port for discovery broadcasts

let mainWindow;

// When Electron app is ready
app.whenReady().then(() => {
  // Create the main application window
  mainWindow = createWindow();

  // Register IPC handler for file selection dialog
  ipcMain.handle("open-file-select-dialog", async () => {
    const result = await dialog.showOpenDialog(mainWindow, {
      properties: ["openFile", "multiSelections"],
    });
    if (result.canceled) return [];
    return result.filePaths.map((filePath) => {
      let size = 0;
      try {
        size = fs.statSync(filePath).size;
      } catch (e) {
        /* ignore size read errors */
      }
      return {
        path: filePath,
        name: path.basename(filePath),
        size,
      };
    });
  });

  // Start network services for peer discovery and file transfer
  startBonjour(mainWindow, serviceName, SERVICE_TYPE, PORT); // mDNS discovery
  startUdpDiscovery(mainWindow, serviceName, PORT, UDP_PORT); // UDP broadcast discovery
  startTcpServer(mainWindow, PORT); // TCP server for receiving files
  setupFileSender(mainWindow); // Setup IPC handlers for sending files
});

// Quit the app when all windows are closed (except on macOS)
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
