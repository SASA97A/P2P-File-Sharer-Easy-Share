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
  mainWindow.webContents.openDevTools();
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
    if (mainWindow) {
      mainWindow.webContents.send("peer-lost", {
        name: service.name,
        host: service.referer.address,
        port: service.port,
      });
    }
  });
}

const peerLastSeen = {};

function startUdpDiscovery() {
  const udp = dgram.createSocket("udp4");

  // Listen for broadcasts
  udp.on("message", (msg, rinfo) => {
    try {
      const peer = JSON.parse(msg.toString());
      if (peer.name === serviceName) return;

      // Track last seen time
      const key = `${peer.host}:${peer.port}`;
      peerLastSeen[key] = Date.now();

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

  // Cleanup stale peers every 15s
  setInterval(() => {
    const now = Date.now();
    for (const key in peerLastSeen) {
      if (now - peerLastSeen[key] > 15000) {
        // not seen in 15s
        delete peerLastSeen[key];
        if (mainWindow) {
          const [host, port] = key.split(":");
          mainWindow.webContents.send("peer-lost", {
            host,
            port: Number(port),
          });
        }
      }
    }
  }, 5000);
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

    let metaLength = null;
    let metaBuffer = Buffer.alloc(0);
    let fileStream = null;
    let expectedFileSize = 0;
    let receivedFileSize = 0;

    socket.on("data", async (chunk) => {
      try {
        // If we haven't parsed metadata yet
        if (metaLength === null) {
          // Not enough data yet to read the 4-byte length
          if (metaBuffer.length + chunk.length < 4) {
            metaBuffer = Buffer.concat([metaBuffer, chunk]);
            return;
          }

          // Combine and extract metadata length
          metaBuffer = Buffer.concat([metaBuffer, chunk]);
          metaLength = metaBuffer.readUInt32BE(0);

          // Still waiting for full metadata
          if (metaBuffer.length < 4 + metaLength) return;

          // Extract metadata JSON
          const metadata = JSON.parse(
            metaBuffer.slice(4, 4 + metaLength).toString()
          );
          expectedFileSize = metadata.size;

          // Remaining buffer after metadata (may contain file data)
          const remaining = metaBuffer.slice(4 + metaLength);

          // Ask user for save location
          const { canceled, filePaths } = await dialog.showOpenDialog(
            mainWindow,
            {
              title: "Choose folder to save received file",
              properties: ["openDirectory"],
            }
          );
          if (canceled || filePaths.length === 0) {
            socket.destroy();
            return;
          }

          const saveDir = filePaths[0];
          let filePath = path.join(saveDir, metadata.name);

          // Prevent overwrite
          let counter = 1;
          while (fs.existsSync(filePath)) {
            const parsed = path.parse(filePath);
            filePath = path.join(
              parsed.dir,
              `${parsed.name}(${counter})${parsed.ext}`
            );
            counter++;
          }

          fileStream = fs.createWriteStream(filePath);

          // Write remaining data (after metadata) to file
          if (remaining.length > 0) {
            fileStream.write(remaining);
            receivedFileSize += remaining.length;
          }
        } else {
          // Already past metadata → write chunks directly
          fileStream.write(chunk);
          receivedFileSize += chunk.length;
        }
      } catch (err) {
        console.error("Error handling chunk:", err);
        socket.destroy();
      }
    });

    socket.on("end", () => {
      if (fileStream) {
        fileStream.end();
        console.log(
          `File received (${receivedFileSize}/${expectedFileSize} bytes)`
        );

        mainWindow.webContents.send("file-received", "File saved successfully");
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

ipcMain.handle("send-file", async (event, peer, fileObj) => {
  return new Promise((resolve, reject) => {
    try {
      const { name, size, path: filePath } = fileObj;

      // Metadata buffer
      const metadata = JSON.stringify({ name, size });
      const metaBuffer = Buffer.from(metadata);
      const metaLengthBuffer = Buffer.alloc(4);
      metaLengthBuffer.writeUInt32BE(metaBuffer.length, 0);

      const client = new net.Socket();
      let bytesSent = 0;

      client.connect(peer.port, peer.host, () => {
        console.log(`Connected to ${peer.name} at ${peer.host}:${peer.port}`);

        // Send metadata first
        client.write(metaLengthBuffer);
        client.write(metaBuffer);

        // Then stream file
        const fileStream = fs.createReadStream(filePath, {
          highWaterMark: 64 * 1024, // 64KB chunks
        });

        fileStream.on("data", (chunk) => {
          bytesSent += chunk.length;

          if (!client.write(chunk)) {
            fileStream.pause(); // backpressure
            client.once("drain", () => fileStream.resume());
          }

          // Progress update
          mainWindow.webContents.send("send-progress", {
            peer,
            sent: bytesSent,
            total: size,
          });
        });

        fileStream.on("end", () => {
          client.end();
        });

        fileStream.on("error", (err) => {
          reject("File read error: " + err.message);
        });
      });

      client.on("close", () => {
        resolve("File sent successfully!");
      });

      client.on("error", (err) => {
        reject("Error sending file: " + err.message);
      });
    } catch (err) {
      reject("Unexpected error: " + err.message);
    }
  });
});
