// Import Node.js UDP module for network discovery
const dgram = require("dgram");
const os = require("os");

// Track last seen time for each peer
const peerLastSeen = {};

/**
 * Gets the local IP address of the machine
 * @returns {string} Local IP address or 127.0.0.1 if none found
 */
function getLocalIp() {
  const nets = os.networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name]) {
      // Find the first non-internal IPv4 address
      if (net.family === "IPv4" && !net.internal) return net.address;
    }
  }
  return "127.0.0.1"; // Fallback to localhost
}

/**
 * Starts UDP-based peer discovery as a backup to Bonjour/mDNS
 * @param {BrowserWindow} mainWindow - Main application window
 * @param {string} serviceName - Name to advertise (computer hostname)
 * @param {number} PORT - TCP port for file transfer
 * @param {number} UDP_PORT - UDP port for discovery broadcasts
 */
function startUdpDiscovery(mainWindow, serviceName, PORT, UDP_PORT) {
  const udp = dgram.createSocket("udp4");

  // Handle incoming UDP discovery messages
  udp.on("message", (msg) => {
    try {
      const peer = JSON.parse(msg.toString());
      if (peer.name === serviceName) return; // Ignore our own broadcasts

      // Track when we last saw this peer
      const key = `${peer.host}:${peer.port}`;
      peerLastSeen[key] = Date.now();

      // Notify renderer about discovered peer
      mainWindow.webContents.send("peer-found", peer);
    } catch (err) {
      console.error("Invalid UDP message:", err);
    }
  });

  // Start listening for UDP broadcasts
  udp.bind(UDP_PORT, () => {
    udp.setBroadcast(true);

    // Broadcast our presence every 5 seconds
    // Clean up peers that haven't been seen for 15 seconds
    setInterval(() => {
      const peer = JSON.stringify({
        name: serviceName,
        host: getLocalIp(),
        port: PORT,
        osType: os.type(),
      });
      // Send to broadcast address
      udp.send(peer, 0, peer.length, UDP_PORT, "255.255.255.255");
    }, 5000);
  });

  setInterval(() => {
    const now = Date.now();
    for (const key in peerLastSeen) {
      if (now - peerLastSeen[key] > 15000) {
        delete peerLastSeen[key];
        const [host, port] = key.split(":");
        // Notify renderer about lost peer
        mainWindow.webContents.send("peer-lost", {
          host,
          port: Number(port),
        });
      }
    }
  }, 5000);
}

module.exports = { startUdpDiscovery };
