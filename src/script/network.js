/**
 * Easy Share - Network Selector
 * Lets the user choose which network's address other devices use to reach this one.
 * Hidden on devices with a single network, where there is nothing to choose.
 */

import { getNetworkOptions, setNetworkAdapter, getMyPairingInfo } from "./tauri-bridge.js";
import { showToast } from "./ui.js";

const SELECTORS = [
  { field: "settingsNetworkField", select: "settingsNetworkSelect" },
  { field: "pairNetworkField", select: "pairNetworkSelect" },
];

let currentNetworks = { options: [], selected: null };

/**
 * Renders the network choices into every selector
 * @param {{options: Array<{adapter: string, ip: string}>, selected: string|null}} networks
 */
export function renderNetworkOptions(networks) {
  currentNetworks = networks;
  const options = networks?.options || [];

  SELECTORS.forEach(({ field, select }) => {
    const fieldEl = document.getElementById(field);
    const selectEl = document.getElementById(select);
    if (!fieldEl || !selectEl) return;

    fieldEl.classList.toggle("hidden", options.length < 2);
    selectEl.replaceChildren(
      ...options.map(({ adapter, ip }) => {
        const option = document.createElement("option");
        option.value = adapter;
        option.textContent = `${adapter} · ${ip}`;
        return option;
      })
    );
    selectEl.value = networks?.selected || "";
  });
}

/**
 * Re-reads the available networks, which change as adapters connect and disconnect
 */
export async function refreshNetworkOptions() {
  try {
    renderNetworkOptions(await getNetworkOptions());
  } catch (err) {
    console.error("Failed to load networks:", err);
  }
}

/**
 * Wires the selectors and loads the initial choices
 * @param {(pairing: Object) => void} onAddressChanged receives the updated pairing info
 */
export async function setupNetworkSelector(onAddressChanged) {
  SELECTORS.forEach(({ select }) => {
    const selectEl = document.getElementById(select);
    if (!selectEl) return;

    selectEl.addEventListener("change", async () => {
      try {
        renderNetworkOptions(await setNetworkAdapter(selectEl.value));
        onAddressChanged(await getMyPairingInfo());
      } catch (err) {
        console.error("Failed to change network:", err);
        showToast(`Failed to change network: ${err}`, "error");
        renderNetworkOptions(currentNetworks);
      }
    });
  });

  await refreshNetworkOptions();
}
