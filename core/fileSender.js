// Import required Node.js modules
const net = require("net");
const { ipcMain } = require("electron");

/**
 * Sets up IPC handlers for sending files to other devices
 * @param {BrowserWindow} mainWindow - Main application window
 */
function setupFileSender(mainWindow) {
  ipcMain.handle("send-file", async (event, peer, fileObj) => {
    return new Promise((resolve, reject) => {
      try {
        const { name, size, data } = fileObj;
        const fileBuffer = Buffer.from(data);

        const metadata = JSON.stringify({ name, size });
        const metaBuffer = Buffer.from(metadata);
        const metaLengthBuffer = Buffer.alloc(4);
        metaLengthBuffer.writeUInt32BE(metaBuffer.length, 0);

        const client = new net.Socket();
        let offset = 0;

        client.connect(peer.port, peer.host, () => {
          console.log(`Connected to ${peer.name}`);

          client.write(metaLengthBuffer);
          client.write(metaBuffer);

          const CHUNK_SIZE = 64 * 1024; // 64KB chunks

          // Recursive function to send file in chunks
          function sendNext() {
            if (offset >= fileBuffer.length) {
              client.end();
              return;
            }
            const chunk = fileBuffer.slice(offset, offset + CHUNK_SIZE);
            offset += chunk.length;

            // Handle backpressure - wait for drain if buffer is full
            if (!client.write(chunk)) {
              client.once("drain", sendNext);
            } else {
              sendNext();
            }

            // Update progress in renderer
            mainWindow.webContents.send("send-progress", {
              peer,
              sent: offset,
              total: size,
            });
          }

          sendNext();
        });

        client.on("close", () => resolve("File sent successfully!"));
        client.on("error", (err) =>
          reject("Error sending file: " + err.message)
        );
      } catch (err) {
        reject("Unexpected error: " + err.message);
      }
    });
  });
}

module.exports = { setupFileSender };
