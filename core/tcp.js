const net = require("net");
const fs = require("fs");
const path = require("path");
const { dialog } = require("electron");

function startTcpServer(mainWindow, PORT) {
  const server = net.createServer((socket) => {
    console.log("Incoming connection:", socket.remoteAddress);

    let metaLength = null;
    let metadataBuffer = null;
    let metaBytesRead = 0;
    let fileStream = null;
    let expectedFileSize = 0;
    let receivedFileSize = 0;

    socket.on("data", async (chunk) => {
      let offset = 0;

      while (offset < chunk.length) {
        if (metaLength === null && chunk.length - offset >= 4) {
          metaLength = chunk.readUInt32BE(offset);
          metadataBuffer = Buffer.alloc(metaLength);
          metaBytesRead = 0;
          offset += 4;
        }

        if (metaLength !== null && fileStream === null) {
          const remainingMeta = metaLength - metaBytesRead;
          const available = chunk.length - offset;
          const toCopy = Math.min(remainingMeta, available);

          chunk.copy(metadataBuffer, metaBytesRead, offset, offset + toCopy);
          metaBytesRead += toCopy;
          offset += toCopy;

          if (metaBytesRead === metaLength) {
            const metadata = JSON.parse(metadataBuffer.toString());
            expectedFileSize = metadata.size;

            socket.pause();
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

            let savePath = path.join(filePaths[0], metadata.name);
            let counter = 1;
            while (fs.existsSync(savePath)) {
              const parsed = path.parse(savePath);
              savePath = path.join(
                parsed.dir,
                `${parsed.name}(${counter})${parsed.ext}`
              );
              counter++;
            }

            fileStream = fs.createWriteStream(savePath);
            socket.resume();
          }
        }

        if (fileStream) {
          const toWrite = chunk.length - offset;
          if (toWrite > 0) {
            const fileChunk = chunk.slice(offset, offset + toWrite);
            offset += toWrite;

            receivedFileSize += fileChunk.length;
            if (!fileStream.write(fileChunk)) {
              socket.pause();
              fileStream.once("drain", () => socket.resume());
            }
          }
        }
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

  server.listen(PORT, () => console.log(`TCP server running on port ${PORT}`));
}

module.exports = { startTcpServer };
