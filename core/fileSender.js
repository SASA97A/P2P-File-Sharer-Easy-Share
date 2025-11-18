// Import required Node.js modules
const net = require("net");
const fs = require("fs");
const { ipcMain } = require("electron");

/**
 * Sets up IPC handlers for sending files to other devices
 * @param {BrowserWindow} mainWindow - Main application window
 */
function setupFileSender(mainWindow) {
  ipcMain.handle("send-file", async (event, peer, fileObj) => {
    return new Promise((resolve, reject) => {
      // Destructure the expected fullPath property from the renderer
      const { name, size, fullPath } = fileObj;

      // 🎯 CRITICAL FIX: Explicitly check for the missing path.
      // This is what caused "Received undefined" and the subsequent fs error.
      if (!fullPath) {
        console.error(
          "IPC Error: File path is missing from the renderer file object."
        );
        return reject(
          "Error: Cannot start file stream. The file's disk path is unavailable. Please ensure 'files.js' is correctly extracting the 'file.path' property."
        );
      }

      try {
        // Create a read stream directly from the source file path
        const fileStream = fs.createReadStream(fullPath);

        // Prepare metadata
        const metadata = JSON.stringify({ name, size });
        const metaBuffer = Buffer.from(metadata);

        // PROTOCOL ENHANCEMENT: Use 64-bit integer for metadata length (8 bytes)
        const metaLengthBuffer = Buffer.alloc(8);
        metaLengthBuffer.writeBigUInt64BE(BigInt(metaBuffer.length), 0);

        const client = new net.Socket();
        let bytesSent = 0;

        client.connect(peer.port, peer.host, () => {
          console.log(`Connected to ${peer.name}`);

          // Send 64-bit metadata length header first
          client.write(metaLengthBuffer);
          client.write(metaBuffer);

          // Pipe the file stream to the socket with proper handling
          fileStream.on("data", (chunk) => {
            const canContinue = client.write(chunk);
            bytesSent += chunk.length;

            // Update progress in renderer
            mainWindow.webContents.send("send-progress", {
              peer,
              sent: bytesSent,
              total: size,
            });

            // Handle backpressure
            if (!canContinue) {
              fileStream.pause();
              client.once("drain", () => {
                fileStream.resume();
              });
            }
          });

          fileStream.on("end", () => {
            client.end();
          });

          fileStream.on("error", (err) => {
            client.destroy();
            reject("Error reading file: " + err.message);
          });
        });

        client.on("close", () => {
          fileStream.destroy();
          resolve("File sent successfully!");
        });

        client.on("error", (err) => {
          fileStream.destroy();
          reject("Error sending file: " + err.message);
        });
      } catch (err) {
        // Catch errors like bad path format
        reject("Unexpected stream error: " + err.message);
      }
    });
  });
}

module.exports = { setupFileSender };
