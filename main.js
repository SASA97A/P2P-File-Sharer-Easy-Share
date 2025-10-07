// Import required Electron modules
const { app, ipcMain, dialog } = require("electron");
const os = require("os");
const path = require("path");
const fs = require("fs");
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

// Default download location
let downloadLocation = path.join(os.homedir(), "Downloads");

let mainWindow;

// When Electron app is ready
app.whenReady().then(() => {
  // Create the main application window
  mainWindow = createWindow();

  // Start network services for peer discovery and file transfer
  startBonjour(mainWindow, serviceName, SERVICE_TYPE, PORT); // mDNS discovery
  startUdpDiscovery(mainWindow, serviceName, PORT, UDP_PORT); // UDP broadcast discovery
  startTcpServer(mainWindow, PORT, downloadLocation); // TCP server for receiving files
  setupFileSender(mainWindow); // Setup IPC handlers for sending files
  
  // Setup IPC handlers for download location
  ipcMain.handle("choose-download-location", async () => {
    const { canceled, filePaths } = await dialog.showOpenDialog(mainWindow, {
      title: "Choose Download Location",
      properties: ["openDirectory"],
    });
    
    if (!canceled && filePaths.length > 0) {
      downloadLocation = filePaths[0];
      return downloadLocation;
    }
    return null;
  });
  
  ipcMain.handle("get-download-location", () => {
    return downloadLocation;
  });
});

// Quit the app when all windows are closed (except on macOS)
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
