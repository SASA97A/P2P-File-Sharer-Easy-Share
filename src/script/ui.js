/**
 * Renders the list of discovered peers in the UI
 * @param {Array} peers - Array of peer objects
 * @param {Function} onSelectPeer - Callback when a peer is selected
 */
export function renderPeers(peers, onSelectPeer) {
  const deviceList = document.getElementById("deviceList");
  deviceList.innerHTML = "";

  // Show loading indicator if no peers found
  if (peers.length === 0) {
    const loader = document.createElement("div");
    loader.className = "loader";
    loader.textContent = "Searching for devices...";
    deviceList.appendChild(loader);
    return;
  }

  // Create UI elements for each peer
  peers.forEach((peer) => {
    const div = document.createElement("div");
    div.className = "device";
    div.innerHTML = `
      <div class="device-icon">🖥️</div>
      <div class="device-name">${peer.name}</div>
      <div class="checkmark">✔</div>
    `;

    // Handle peer selection
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

/**
 * Updates the file list in the UI
 * @param {Array} files - Array of File objects
 * @param {Function} removeFileCb - Callback to remove a file
 */
export function updateFileList(files, removeFileCb) {
  const fileList = document.getElementById("fileList");
  fileList.innerHTML = "";

  // Create UI elements for each file
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

  // Add event listeners to remove buttons
  fileList.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      removeFileCb(e.target.dataset.index);
    });
  });
}

/**
 * Formats file size in human-readable format (KB, MB, GB)
 * @param {number} bytes - File size in bytes
 * @returns {string} Formatted file size
 */
function formatFileSize(bytes) {
  if (bytes >= 1024 ** 3) {
    return (bytes / 1024 ** 3).toFixed(2) + " GB";
  } else if (bytes >= 1024 ** 2) {
    return (bytes / 1024 ** 2).toFixed(2) + " MB";
  } else {
    return (bytes / 1024).toFixed(2) + " KB";
  }
}

/**
 * Shows a toast notification
 * @param {string} message - Message to display
 * @param {string} type - Type of toast (info, success, error)
 */
export function showToast(message, type = "info") {
  const container = document.getElementById("toast-container");
  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.textContent = message;
  container.appendChild(toast);

  // Auto-remove toast after 3 seconds
  setTimeout(() => {
    toast.remove();
  }, 3000);
}
