const net = require("net");
const { ipcMain } = require("electron");

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

          const CHUNK_SIZE = 64 * 1024;

          function sendNext() {
            if (offset >= fileBuffer.length) {
              client.end();
              return;
            }
            const chunk = fileBuffer.slice(offset, offset + CHUNK_SIZE);
            offset += chunk.length;

            if (!client.write(chunk)) {
              client.once("drain", sendNext);
            } else {
              sendNext();
            }

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
