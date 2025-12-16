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
 */
function startTcpServer(mainWindow, PORT) {
  const server = net.createServer((socket) => {
    console.log("Incoming connection:", socket.remoteAddress);

    // Variables to track file transfer state
    let metaLength = null; // Length of metadata JSON (must be 64-bit compatible)
    let metadataBuffer = null; // Buffer to store metadata
    let metaBytesRead = 0; // How many bytes of metadata we've read
    let fileStream = null; // Stream to write file to disk
    let expectedFileSize = 0; // Total file size from metadata
    let receivedFileSize = 0; // How many bytes we've received so far

    // Constant for the length of the header
    const META_LENGTH_HEADER_SIZE = 8;

    let streamClosed = false;

    socket.on("data", async (chunk) => {
      let offset = 0;

      while (offset < chunk.length) {
        // Step 1: Read metadata length (8 bytes)
        if (
          metaLength === null &&
          chunk.length - offset >= META_LENGTH_HEADER_SIZE
        ) {
          // Read the 8-byte length as a BigInt, then convert to Number for use
          const bigMetaLength = chunk.readBigUInt64BE(offset);
          metaLength = Number(bigMetaLength);

          metadataBuffer = Buffer.alloc(metaLength);
          metaBytesRead = 0;
          offset += META_LENGTH_HEADER_SIZE; // 🎯 FIX: Increase offset by 8 bytes
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
            // This is the line that was failing:
            const metadata = JSON.parse(metadataBuffer.toString());

            expectedFileSize = metadata.size;

            // Get the Downloads folder path
            const downloadsPath = path.join(os.homedir(), "Downloads");

            // Create a unique filename if file already exists
            let savePath = path.join(downloadsPath, metadata.name);
            let counter = 1;
            while (fs.existsSync(savePath)) {
              const parsed = path.parse(savePath);
              // NOTE: This assumes 'currentSavePath' is available for logging later
              savePath = path.join(
                parsed.dir,
                `${parsed.name}(${counter})${parsed.ext}`
              );
              counter++;
            }

            // Create write stream and resume socket
            fileStream = fs.createWriteStream(savePath);
            // Store the savePath to send back to the renderer (Assuming 'currentSavePath' is defined outside)
            let currentSavePath = savePath;
          }
        }

        // Step 3: Write file data to disk
        if (fileStream && !streamClosed) {
          const toWrite = chunk.length - offset;
          if (toWrite > 0) {
            const fileChunk = chunk.slice(offset, offset + toWrite);
            offset += toWrite;

            receivedFileSize += fileChunk.length;

            if (!fileStream.destroyed) {
              if (!fileStream.write(fileChunk)) {
                socket.pause();
                fileStream.once("drain", () => socket.resume());
              }
            }
          }
        }
      }
    });

    // When transfer is complete
    socket.on("end", () => {
      if (fileStream && !streamClosed) {
        streamClosed = true;
        fileStream.end();
        console.log(
          `File received (${receivedFileSize}/${expectedFileSize} bytes)`
        );
        // Ensure to pass the correct success path back to the renderer if needed
        mainWindow.webContents.send("file-received", "File saved successfully");
      }
    });

    socket.on("error", (err) => {
      console.error("Socket error:", err);
    });

    // Handle socket timeout/close for cleanup
    socket.on("close", () => {
      if (fileStream && !streamClosed) {
        streamClosed = true;
        fileStream.destroy();
      }
    });
  });

  server.listen(PORT, () => console.log(`TCP server running on port ${PORT}`));
}

module.exports = { startTcpServer };
