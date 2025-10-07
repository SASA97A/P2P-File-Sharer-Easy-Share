// Import required Electron modules
const { BrowserWindow } = require("electron");
const path = require("path");

let mainWindow;

/**
 * Creates and configures the main application window
 * @returns {BrowserWindow} The created window instance
 */
function createWindow() {
  mainWindow = new BrowserWindow({
    width: 800,
    height: 550,
    webPreferences: {
      preload: path.join(__dirname, "../preload.js"), // Load preload script for IPC
      contextIsolation: true, // Isolate renderer process from Node.js
      nodeIntegration: false, // Disable direct Node.js access from renderer
    },
  });

  // Load the main HTML file
  mainWindow.loadFile("src/index.html");
  // Hide the default menu bar
  mainWindow.setMenuBarVisibility(false);
  // Open DevTools for debugging
  mainWindow.webContents.openDevTools();

  return mainWindow;
}

module.exports = { createWindow };
