// Import Bonjour for mDNS service discovery
const Bonjour = require("bonjour");
const os = require("os");

/**
 * Starts Bonjour/mDNS service for local network device discovery
 * @param {BrowserWindow} mainWindow - Main application window
 * @param {string} serviceName - Name to advertise (computer hostname)
 * @param {string} SERVICE_TYPE - Type of service to advertise and discover
 * @param {number} PORT - TCP port for the service
 */
function startBonjour(mainWindow, serviceName, SERVICE_TYPE, PORT) {
  const bonjour = Bonjour();

  // Announce this device on the network
  bonjour.publish({
    name: serviceName,
    type: SERVICE_TYPE,
    port: PORT,
    txt: { osType: os.type() },
  });

  // Discover other devices on the network
  const browser = bonjour.find({ type: SERVICE_TYPE });
  
  // When a new device is discovered
  browser.on("up", (service) => {
    // Ignore our own service
    if (service.name === serviceName) return;
    // Send peer info to the renderer process
    mainWindow.webContents.send("peer-found", {
      name: service.name,
      host: service.referer.address,
      port: service.port,
    });
  });

  // When a device disappears from the network
  browser.on("down", (service) => {
    // Send peer lost info to the renderer process
    mainWindow.webContents.send("peer-lost", {
      name: service.name,
      host: service.referer.address,
      port: service.port,
    });
  });
}

module.exports = { startBonjour };
