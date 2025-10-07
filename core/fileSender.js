// Import required Node.js modules
const net = require("net");
const fs = require("fs");
const path = require("path");
const os = require("os");
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
        
        // Create a temporary file path
        const tempDir = os.tmpdir();
        const tempFilePath = path.join(tempDir, `temp_${Date.now()}_${name}`);
        
        // Write the buffer to a temporary file
        fs.writeFileSync(tempFilePath, Buffer.from(data));
        
        // Create a read stream from the temporary file
        const fileStream = fs.createReadStream(tempFilePath);
        
        // Prepare metadata
        const metadata = JSON.stringify({ name, size });
        const metaBuffer = Buffer.from(metadata);
        const metaLengthBuffer = Buffer.alloc(4);
        metaLengthBuffer.writeUInt32BE(metaBuffer.length, 0);
        
        const client = new net.Socket();
        let bytesSent = 0;
        
        client.connect(peer.port, peer.host, () => {
          console.log(`Connected to ${peer.name}`);
          
          // Send metadata first
          client.write(metaLengthBuffer);
          client.write(metaBuffer);
          
          // Pipe the file stream to the socket with proper handling
          fileStream.on('data', (chunk) => {
            // Check if socket can accept more data
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
              client.once('drain', () => {
                fileStream.resume();
              });
            }
          });
          
          fileStream.on('end', () => {
            client.end();
          });
          
          fileStream.on('error', (err) => {
            client.destroy();
            reject("Error reading file: " + err.message);
          });
        });
        
        client.on("close", () => {
          fileStream.destroy();
          // Clean up the temporary file
          try {
            fs.unlinkSync(tempFilePath);
          } catch (err) {
            console.error("Error deleting temp file:", err);
          }
          resolve("File sent successfully!");
        });
        
        client.on("error", (err) => {
          fileStream.destroy();
          // Clean up the temporary file
          try {
            fs.unlinkSync(tempFilePath);
          } catch (cleanupErr) {
            console.error("Error deleting temp file:", cleanupErr);
          }
          reject("Error sending file: " + err.message);
        });
      } catch (err) {
        reject("Unexpected error: " + err.message);
      }
    });
  });
}

module.exports = { setupFileSender };
