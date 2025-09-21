const dropArea = document.getElementById("drop-area");
const fileInput = document.getElementById("fileInput");
const fileList = document.getElementById("fileList");
const deviceList = document.getElementById("deviceList");
const sendBtn = document.querySelector(".send-btn");
const progressBar = document.getElementById("progressBar");

let selectedFiles = [];
let discoveredPeers = [];
let selectedPeer = null;

renderPeers(); // show "Searching..." on startup

// File handling
dropArea.addEventListener("click", () => fileInput.click());
dropArea.addEventListener("dragover", (e) => {
  e.preventDefault();
  dropArea.style.background = "#f0f0f0";
});
dropArea.addEventListener("dragleave", () => {
  dropArea.style.background = "transparent";
});
dropArea.addEventListener("drop", (e) => {
  e.preventDefault();
  addFiles(e.dataTransfer.files);
  dropArea.style.background = "transparent";
});
fileInput.addEventListener("change", () => addFiles(fileInput.files));

function addFiles(files) {
  for (const file of files) {
    selectedFiles.push(file);
  }
  updateFileList();
}

function updateFileList() {
  fileList.innerHTML = "";
  selectedFiles.forEach((file, index) => {
    const item = document.createElement("div");
    item.className = "file-item";
    item.innerHTML = `
      <span>${file.name}</span>
      <span>${(file.size / 1024).toFixed(1)} KB</span>
      <button class="remove-btn" data-index="${index}">✖</button>
    `;
    fileList.appendChild(item);
  });

  // Attach remove listeners
  document.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      const index = e.target.dataset.index;
      selectedFiles.splice(index, 1);
      updateFileList();
    });
  });
}

// ===== Handle peers from Bonjour =====
window.api.onPeerFound((peer) => {
  // Check if already in list
  const exists = discoveredPeers.some(
    (p) => p.host === peer.host && p.port === peer.port
  );

  if (!exists) {
    discoveredPeers.push(peer);
    renderPeers();
  }
});

// Remove peers
window.api.onPeerLost((peer) => {
  discoveredPeers = discoveredPeers.filter(
    (p) => !(p.host === peer.host && p.port === peer.port)
  );
  renderPeers();
});

function renderPeers() {
  deviceList.innerHTML = "";

  if (discoveredPeers.length === 0) {
    // Show loader if no peers found
    const loader = document.createElement("div");
    loader.className = "loader";
    loader.textContent = "Searching for devices...";
    deviceList.appendChild(loader);
    return;
  }

  discoveredPeers.forEach((peer, index) => {
    const div = document.createElement("div");
    div.className = "device";
    div.innerHTML = `<div class="device-icon">🖥️</div><div class="device-name">${peer.name}</div>`;
    div.addEventListener("click", () => {
      selectedPeer = peer;
      document
        .querySelectorAll(".device")
        .forEach((d) => (d.style.border = "none"));
      div.style.border = "2px solid #4caf50";
    });
    deviceList.appendChild(div);
  });
}

/*
function getDeviceImg(type) {
  const icons = {
    Windows_NT: "🖥️",
    Darwin: "💻",
    Linux: "🐧",
  };
  return icons[type] || "📱";
}
*/

// ===== Handle files =====
fileInput.addEventListener("change", () => addFiles(fileInput.files));

function addFiles(files) {
  for (const file of files) {
    selectedFiles.push(file);
  }
  updateFileList();
}

function updateFileList() {
  fileList.innerHTML = "";
  selectedFiles.forEach((file, index) => {
    const item = document.createElement("div");
    item.className = "file-item";
    item.innerHTML = `
      <span>${file.name}</span>
      <span>${(file.size / 1024).toFixed(1)} KB</span>
      <button class="remove-btn" data-index="${index}">✖</button>
    `;
    fileList.appendChild(item);
  });

  document.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      const index = e.target.dataset.index;
      selectedFiles.splice(index, 1);
      updateFileList();
    });
  });
}

// ===== Send File =====
sendBtn.addEventListener("click", async () => {
  if (!selectedPeer) {
    alert("Please select a device first!");
    return;
  }
  if (selectedFiles.length === 0) {
    alert("Please choose a file(s) to send!");
    return;
  }

  try {
    progressBar.style.width = "0%";

    // Send files one by one
    for (let i = 0; i < selectedFiles.length; i++) {
      const file = selectedFiles[i];
      const buffer = await file.arrayBuffer();

      // Metadata for this file
      const metadata = JSON.stringify({ name: file.name, size: file.size });
      const metaBuffer = Buffer.from(metadata);
      const metaLengthBuffer = Buffer.alloc(4);
      metaLengthBuffer.writeUInt32BE(metaBuffer.length, 0);

      // Final buffer: [meta length][metadata][file bytes]
      const finalBuffer = Buffer.concat([
        metaLengthBuffer,
        metaBuffer,
        Buffer.from(buffer),
      ]);

      // Send via IPC
      await window.api.sendFile(selectedPeer, finalBuffer);

      // Update progress (simple per-file percentage)
      const percent = Math.round(((i + 1) / selectedFiles.length) * 100);
      progressBar.style.width = percent + "%";
    }

    alert("All files sent successfully!");
    progressBar.style.width = "100%";
  } catch (err) {
    alert("Error sending files: " + err);
  }
});

// ===== Receive File Event =====
window.api.onFileReceived((filePath) => {
  alert("File received and saved at: " + filePath);
});

// Manual peer add
document.getElementById("manualAddBtn").addEventListener("click", () => {
  const host = document.getElementById("manualIp").value;
  if (!host) return;

  const peer = { name: "Manual", host, port: 5050 };
  discoveredPeers.push(peer);
  renderPeers();
});
