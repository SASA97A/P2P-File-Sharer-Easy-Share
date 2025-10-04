const Bonjour = require("bonjour");
const os = require("os");

function startBonjour(mainWindow, serviceName, SERVICE_TYPE, PORT) {
  const bonjour = Bonjour();

  // Announce this device
  bonjour.publish({
    name: serviceName,
    type: SERVICE_TYPE,
    port: PORT,
    txt: { osType: os.type() },
  });

  // Discover peers
  const browser = bonjour.find({ type: SERVICE_TYPE });
  browser.on("up", (service) => {
    if (service.name === serviceName) return;
    mainWindow.webContents.send("peer-found", {
      name: service.name,
      host: service.referer.address,
      port: service.port,
    });
  });

  browser.on("down", (service) => {
    mainWindow.webContents.send("peer-lost", {
      name: service.name,
      host: service.referer.address,
      port: service.port,
    });
  });
}

module.exports = { startBonjour };
