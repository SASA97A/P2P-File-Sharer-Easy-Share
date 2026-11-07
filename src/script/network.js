/**
 * Easy Share - Network Selector
 * Lets the user choose which network's address other devices use to reach this one.
 * Hidden on devices with a single network, where there is nothing to choose.
 */

import { getNetworkOptions, setNetworkAdapter, getMyPairingInfo } from "./tauri-bridge.js";
import { getIconSvg } from "./icons.js";
import { showToast } from "./ui.js";

const SELECTORS = [
  { field: "settingsNetworkField", trigger: "settingsNetworkBtn", name: "settingsNetworkName", ip: "settingsNetworkIp" },
  { field: "pairNetworkField", trigger: "pairNetworkBtn", name: "pairNetworkName", ip: "pairNetworkIp" },
];

const MENU_GAP = 6;
const MENU_EDGE = 8;

let currentNetworks = { options: [], selected: null };
let onAddressChanged = () => {};
let menuAnchor = null;

/**
 * Shows the current network on every selector
 * @param {{options: Array<{adapter: string, ip: string}>, selected: string|null}} networks
 */
export function renderNetworkOptions(networks) {
  currentNetworks = networks;
  const options = networks?.options || [];
  const current = options.find((option) => option.adapter === networks?.selected) || options[0];

  SELECTORS.forEach(({ field, name, ip }) => {
    const fieldEl = document.getElementById(field);
    const nameEl = document.getElementById(name);
    const ipEl = document.getElementById(ip);
    if (!fieldEl || !nameEl || !ipEl) return;

    fieldEl.classList.toggle("hidden", options.length < 2);
    nameEl.textContent = current?.adapter || "";
    nameEl.title = current?.adapter || "";
    ipEl.textContent = current?.ip || "";
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
 * Switches this device to a network and reports its new address
 * @param {string} adapter
 */
export async function selectNetwork(adapter) {
  try {
    renderNetworkOptions(await setNetworkAdapter(adapter));
    onAddressChanged(await getMyPairingInfo());
  } catch (err) {
    console.error("Failed to change network:", err);
    showToast(`Failed to change network: ${err}`, "error");
  }
}

function buildMenuOption({ adapter, ip }) {
  const isSelected = adapter === currentNetworks.selected;
  const option = document.createElement("button");
  option.type = "button";
  option.className = `net-opt${isSelected ? " is-selected" : ""}`;
  option.setAttribute("role", "option");
  option.setAttribute("aria-selected", String(isSelected));
  option.dataset.adapter = adapter;

  const check = document.createElement("span");
  check.className = "net-opt-check";
  check.setAttribute("aria-hidden", "true");
  if (isSelected) check.innerHTML = getIconSvg("check", { size: 14 });

  const text = document.createElement("span");
  text.className = "net-opt-text";
  const name = document.createElement("span");
  name.className = "net-opt-name";
  name.textContent = adapter;
  name.title = adapter;
  const address = document.createElement("span");
  address.className = "net-opt-ip mono";
  address.textContent = ip;
  text.append(name, address);

  option.append(check, text);
  return option;
}

function positionMenu(menu, anchor) {
  const rect = anchor.getBoundingClientRect();
  const width = Math.max(rect.width, 240);
  menu.style.width = `${width}px`;
  menu.style.maxHeight = "";

  const below = window.innerHeight - rect.bottom - MENU_GAP - MENU_EDGE;
  const above = rect.top - MENU_GAP - MENU_EDGE;
  const openBelow = menu.offsetHeight <= below || below >= above;
  menu.style.maxHeight = `${Math.max(openBelow ? below : above, 0)}px`;

  const height = menu.offsetHeight;
  menu.style.left = `${Math.max(MENU_EDGE, Math.min(rect.left, window.innerWidth - width - MENU_EDGE))}px`;
  menu.style.top = `${openBelow ? rect.bottom + MENU_GAP : rect.top - MENU_GAP - height}px`;
}

function openMenu(anchor) {
  const menu = document.getElementById("networkMenu");
  if (!menu) return;
  closeMenu();
  menuAnchor = anchor;
  menu.replaceChildren(...currentNetworks.options.map(buildMenuOption));
  menu.classList.remove("hidden");
  positionMenu(menu, anchor);
  anchor.setAttribute("aria-expanded", "true");
  (menu.querySelector(".is-selected") || menu.querySelector(".net-opt"))?.focus();
}

function closeMenu({ restoreFocus = false } = {}) {
  document.getElementById("networkMenu")?.classList.add("hidden");
  if (!menuAnchor) return;
  menuAnchor.setAttribute("aria-expanded", "false");
  if (restoreFocus) menuAnchor.focus();
  menuAnchor = null;
}

function moveFocus(menu, key) {
  const options = [...menu.querySelectorAll(".net-opt")];
  const index = options.indexOf(document.activeElement);
  const targets = {
    ArrowDown: Math.min(index + 1, options.length - 1),
    ArrowUp: Math.max(index - 1, 0),
    Home: 0,
    End: options.length - 1,
  };
  options[targets[key]]?.focus();
}

/**
 * Wires the selectors and loads the initial choices
 * @param {(pairing: Object) => void} addressChanged receives the updated pairing info
 */
export async function setupNetworkSelector(addressChanged) {
  onAddressChanged = addressChanged;
  const menu = document.getElementById("networkMenu");

  SELECTORS.forEach(({ trigger }) => {
    const button = document.getElementById(trigger);
    if (!button) return;

    button.addEventListener("click", (e) => {
      e.stopPropagation();
      if (menuAnchor === button) closeMenu();
      else openMenu(button);
    });
    button.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        openMenu(button);
      }
    });
  });

  if (menu) {
    menu.addEventListener("click", (e) => {
      e.stopPropagation();
      const option = e.target.closest(".net-opt");
      if (!option) return;
      closeMenu({ restoreFocus: true });
      selectNetwork(option.dataset.adapter);
    });
    menu.addEventListener("keydown", (e) => {
      if (["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) {
        e.preventDefault();
        moveFocus(menu, e.key);
      } else if (e.key === "Tab") {
        e.preventDefault();
        closeMenu({ restoreFocus: true });
      }
    });

    document.addEventListener("click", () => closeMenu());
    // Capture phase, so Escape closes only the menu and not the pairing dialog behind it
    document.addEventListener(
      "keydown",
      (e) => {
        if (e.key === "Escape" && menuAnchor) {
          e.stopPropagation();
          closeMenu({ restoreFocus: true });
        }
      },
      true
    );
    // The menu is fixed to the window, so it would detach from a selector that moves
    document.addEventListener(
      "scroll",
      (e) => {
        if (e.target !== menu) closeMenu();
      },
      true
    );
    window.addEventListener("resize", () => closeMenu());
  }

  await refreshNetworkOptions();
}
