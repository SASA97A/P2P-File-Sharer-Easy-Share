/**
 * Sets up the download location settings UI
 */
export function setupSettings() {
  const chooseDownloadBtn = document.getElementById("chooseDownloadBtn");
  const currentDownloadPath = document.getElementById("currentDownloadPath");

  // Initialize with current download location
  window.api.getDownloadLocation().then((location) => {
    if (location) {
      currentDownloadPath.textContent = `Current: ${location}`;
    }
  });

  // Handle download location selection
  chooseDownloadBtn.addEventListener("click", async () => {
    const location = await window.api.chooseDownloadLocation();
    if (location) {
      currentDownloadPath.textContent = `Current: ${location}`;
    }
  });
}