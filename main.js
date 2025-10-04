const { app } = require("electron");
const os = require("os");
const { createWindow } = require("./core/window");
const { startBonjour } = require("./core/bonjour");
const { startUdpDiscovery } = require("./core/udp");
const { startTcpServer } = require("./core/tcp");
const { setupFileSender } = require("./core/fileSender");

const serviceName = os.hostname();
const SERVICE_TYPE = "myfileshare";
const PORT = 5050;
const UDP_PORT = 41234;

let mainWindow;

app.whenReady().then(() => {
  mainWindow = createWindow();

  startBonjour(mainWindow, serviceName, SERVICE_TYPE, PORT);
  startUdpDiscovery(mainWindow, serviceName, PORT, UDP_PORT);
  startTcpServer(mainWindow, PORT);
  setupFileSender(mainWindow);
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
