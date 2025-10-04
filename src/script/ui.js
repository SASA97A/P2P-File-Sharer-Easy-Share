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
      <span>${file.name}</span>
      <span>${(file.size / 1024).toFixed(1)} KB</span>
      <button class="remove-btn" data-index="${index}">✖</button>
    `;
    fileList.appendChild(item);
  });

  fileList.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      removeFileCb(e.target.dataset.index);
    });
  });
}
