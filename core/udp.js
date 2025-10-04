const dgram = require("dgram");
const os = require("os");

const peerLastSeen = {};

function getLocalIp() {
  const nets = os.networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name]) {
      if (net.family === "IPv4" && !net.internal) return net.address;
    }
  }
  return "127.0.0.1";
}

function startUdpDiscovery(mainWindow, serviceName, PORT, UDP_PORT) {
  const udp = dgram.createSocket("udp4");

  udp.on("message", (msg) => {
    try {
      const peer = JSON.parse(msg.toString());
      if (peer.name === serviceName) return;

      const key = `${peer.host}:${peer.port}`;
      peerLastSeen[key] = Date.now();
      mainWindow.webContents.send("peer-found", peer);
    } catch (err) {
      console.error("Invalid UDP message:", err);
    }
  });

  udp.bind(UDP_PORT, () => {
    udp.setBroadcast(true);

    setInterval(() => {
      const peer = JSON.stringify({
        name: serviceName,
        host: getLocalIp(),
        port: PORT,
        osType: os.type(),
      });
      udp.send(peer, 0, peer.length, UDP_PORT, "255.255.255.255");
    }, 5000);
  });

  setInterval(() => {
    const now = Date.now();
    for (const key in peerLastSeen) {
      if (now - peerLastSeen[key] > 15000) {
        delete peerLastSeen[key];
        const [host, port] = key.split(":");
        mainWindow.webContents.send("peer-lost", {
          host,
          port: Number(port),
        });
      }
    }
  }, 5000);
}

module.exports = { startUdpDiscovery };
