// Renders peer list in the UI
export function renderPeers(peers, onSelectPeer) {
  const deviceList = document.getElementById("deviceList");
  deviceList.innerHTML = "";

  if (peers.length === 0) {
    const loader = document.createElement("div");
    loader.className = "loader";
    loader.textContent = "Searching for devices...";
    deviceList.appendChild(loader);
    return;
  }

  peers.forEach((peer) => {
    const div = document.createElement("div");
    div.className = "device";
    div.innerHTML = `
      <div class="device-icon">🖥️</div>
      <div class="device-name">${peer.name}</div>
      <div class="checkmark">✔</div>
    `;

    div.addEventListener("click", () => {
      document
        .querySelectorAll(".device")
        .forEach((d) => d.classList.remove("selected"));
      div.classList.add("selected");
      onSelectPeer(peer);
    });

    deviceList.appendChild(div);
  });
}

// Renders file list in the UI
export function updateFileList(files, removeFileCb) {
  const fileList = document.getElementById("fileList");
  fileList.innerHTML = "";

  files.forEach((file, index) => {
    const item = document.createElement("div");
    item.className = "file-item";

    item.innerHTML = `
      <div class="file-info">
        <span class="file-name" title="${file.name}">${file.name}</span>
        <span class="file-size">${formatFileSize(file.size)}</span>
        <button class="remove-btn" data-index="${index}">✖</button>
      </div>
      <div class="progress-container">
        <div class="progress-bar" id="progress-${index}"></div>
      </div>
    `;

    fileList.appendChild(item);
  });

  fileList.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      removeFileCb(e.target.dataset.index);
    });
  });
}

//Helper to determine file size in KB, MB or GB
function formatFileSize(bytes) {
  if (bytes >= 1024 ** 3) {
    return (bytes / 1024 ** 3).toFixed(2) + " GB";
  } else if (bytes >= 1024 ** 2) {
    return (bytes / 1024 ** 2).toFixed(2) + " MB";
  } else {
    return (bytes / 1024).toFixed(2) + " KB";
  }
}

// Function for toast notifications
export function showToast(message, type = "info") {
  const container = document.getElementById("toast-container");
  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.textContent = message;
  container.appendChild(toast);

  setTimeout(() => {
    toast.remove();
  }, 3000); // disappear after 3s
}
