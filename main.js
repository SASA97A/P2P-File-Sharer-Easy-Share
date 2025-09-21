const { app, BrowserWindow, ipcMain, dialog } = require("electron");
const path = require("path");
const net = require("net");
const fs = require("fs");
const os = require("os");
const dgram = require("dgram");
const Bonjour = require("bonjour");
const bonjour = Bonjour();

let mainWindow;
const serviceName = os.hostname();
const SERVICE_TYPE = "myfileshare";
const PORT = 5050;
const UDP_PORT = 41234; // UDP broadcast port

const createWindow = () => {
  mainWindow = new BrowserWindow({
    width: 800,
    height: 600,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  mainWindow.loadFile("src/index.html");
  mainWindow.setMenuBarVisibility(false);
};

app.whenReady().then(() => {
  createWindow();
  startBonjour();
  startUdpDiscovery();
  startTcpServer();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

function startBonjour() {
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
    if (mainWindow) {
      mainWindow.webContents.send("peer-found", {
        name: service.name,
        host: service.referer.address,
        port: service.port,
      });
    }
  });

  // When peer disappears
  browser.on("down", (service) => {
    discoveredPeers = discoveredPeers.filter(
      (p) => !(p.host === service.referer.address && p.port === service.port)
    );
    renderPeers();
  });
}

function startUdpDiscovery() {
  const udp = dgram.createSocket("udp4");

  // Listen for broadcasts
  udp.on("message", (msg, rinfo) => {
    try {
      const peer = JSON.parse(msg.toString());
      if (peer.name === serviceName) return;

      if (mainWindow) {
        mainWindow.webContents.send("peer-found", peer);
      }
    } catch (err) {
      console.error("Invalid UDP message:", err);
    }
  });

  udp.bind(UDP_PORT, () => {
    udp.setBroadcast(true);

    // Broadcast our presence every 5s
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
}

function getLocalIp() {
  const nets = os.networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name]) {
      if (net.family === "IPv4" && !net.internal) {
        return net.address;
      }
    }
  }
  return "127.0.0.1";
}

function startTcpServer() {
  const server = net.createServer((socket) => {
    console.log("Incoming connection:", socket.remoteAddress);

    let receivedData = Buffer.alloc(0);
    socket.on("data", (chunk) => {
      receivedData = Buffer.concat([receivedData, chunk]);
    });

    socket.on("end", async () => {
      try {
        let offset = 0;

        const metaLength = receivedData.readUInt32BE(offset);
        offset += 4;

        const metaBuffer = receivedData.slice(offset, offset + metaLength);
        const metadata = JSON.parse(metaBuffer.toString());
        offset += metaLength;

        const fileData = receivedData.slice(offset);

        const { canceled, filePaths } = await dialog.showOpenDialog(
          mainWindow,
          {
            title: "Choose folder to save received file",
            properties: ["openDirectory"],
          }
        );
        if (canceled || filePaths.length === 0) return;

        const saveDir = filePaths[0];
        const filePath = path.join(saveDir, metadata.name);

        fs.writeFileSync(filePath, fileData);
        console.log(`File received: ${metadata.name} (${metadata.size} bytes)`);

        mainWindow.webContents.send("file-received", filePath);
      } catch (err) {
        console.error("Error processing received file:", err);
      }
    });

    socket.on("error", (err) => {
      console.error("Socket error:", err);
    });
  });

  server.listen(PORT, () => {
    console.log("TCP server running on port", PORT);
  });
}

ipcMain.handle("send-file", async (event, peer, fileBuffer) => {
  return new Promise((resolve, reject) => {
    try {
      const client = new net.Socket();
      client.connect(peer.port, peer.host, () => {
        client.write(fileBuffer);
        client.end();
      });

      client.on("close", () => resolve("File sent successfully!"));
      client.on("error", (err) => reject("Error sending file: " + err.message));
    } catch (err) {
      reject("Unexpected error: " + err.message);
    }
  });
});
