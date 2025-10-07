// Import required Node.js modules
const net = require("net");
const fs = require("fs");
const path = require("path");
const { dialog } = require("electron");
const os = require("os");

/**
 * Starts a TCP server to receive files from other devices
 * @param {BrowserWindow} mainWindow - Main application window
 * @param {number} PORT - TCP port to listen on
 * @param {string} downloadLocation - Path to save downloaded files
 */
function startTcpServer(mainWindow, PORT, downloadLocation) {
  const server = net.createServer((socket) => {
    console.log("Incoming connection:", socket.remoteAddress);

    // Variables to track file transfer state
    let metaLength = null; // Length of metadata JSON
    let metadataBuffer = null; // Buffer to store metadata
    let metaBytesRead = 0; // How many bytes of metadata we've read
    let fileStream = null; // Stream to write file to disk
    let expectedFileSize = 0; // Total file size from metadata
    let receivedFileSize = 0; // How many bytes we've received so far
    let currentSavePath = ""; // Path where the file is being saved

    socket.on("data", async (chunk) => {
      let offset = 0;

      while (offset < chunk.length) {
        // Step 1: Read metadata length (4 bytes)
        if (metaLength === null && chunk.length - offset >= 4) {
          metaLength = chunk.readUInt32BE(offset);
          metadataBuffer = Buffer.alloc(metaLength);
          metaBytesRead = 0;
          offset += 4;
        }

        // Step 2: Read metadata JSON
        if (metaLength !== null && fileStream === null) {
          const remainingMeta = metaLength - metaBytesRead;
          const available = chunk.length - offset;
          const toCopy = Math.min(remainingMeta, available);

          chunk.copy(metadataBuffer, metaBytesRead, offset, offset + toCopy);
          metaBytesRead += toCopy;
          offset += toCopy;

          // If we've read all metadata, parse it and prepare for file
          if (metaBytesRead === metaLength) {
            const metadata = JSON.parse(metadataBuffer.toString());
            expectedFileSize = metadata.size;

            // Use the configured download location
            // Create a unique filename if file already exists
            let savePath = path.join(downloadLocation, metadata.name);
            let counter = 1;
            while (fs.existsSync(savePath)) {
              const parsed = path.parse(savePath);
              savePath = path.join(
                parsed.dir,
                `${parsed.name}(${counter})${parsed.ext}`
              );
              counter++;
            }

            // Create write stream and resume socket
            fileStream = fs.createWriteStream(savePath);
            currentSavePath = savePath;
          }
        }

        // Step 3: Write file data to disk
        if (fileStream) {
          const toWrite = chunk.length - offset;
          if (toWrite > 0) {
            const fileChunk = chunk.slice(offset, offset + toWrite);
            offset += toWrite;

            receivedFileSize += fileChunk.length;
            // Handle backpressure - pause socket if write stream is full
            if (!fileStream.write(fileChunk)) {
              socket.pause();
              fileStream.once("drain", () => socket.resume());
            }
          }
        }
      }
    });

    // When transfer is complete
    socket.on("end", () => {
      if (fileStream) {
        fileStream.end();
        console.log(
          `File received (${receivedFileSize}/${expectedFileSize} bytes)`
        );
        mainWindow.webContents.send("file-received", currentSavePath);
      }
    });

    socket.on("error", (err) => {
      console.error("Socket error:", err);
    });
  });

  server.listen(PORT, () => console.log(`TCP server running on port ${PORT}`));
}

module.exports = { startTcpServer };
