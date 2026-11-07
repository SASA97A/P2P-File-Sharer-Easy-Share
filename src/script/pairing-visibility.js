/**
 * Easy Share - Pairing Visibility
 * Controls whether other devices can pair with this device's PIN or QR code.
 * Pairing is off by default and only turns on when the user picks 10 or 30 minutes,
 * from the sidebar eye or from the status strip in the pairing dialog.
 */

import { getPairingStatus, setPairingMode } from "./tauri-bridge.js";
import { showToast } from "./ui.js";

const EYE_ON =
  '<svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/></svg>';
const EYE_OFF =
  '<svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/></svg>';
const CHECK =
  '<svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>';

// Local view of the server's pairing window
let endsAt = 0;
let chosenMinutes = 0;
let tickTimer = null;
let menuAnchor = null;

function secondsLeft() {
  return Math.max(0, Math.ceil((endsAt - Date.now()) / 1000));
}

function formatLeft(secs) {
  const m = Math.floor(secs / 60);
  const s = String(secs % 60).padStart(2, "0");
  return `${m}:${s}`;
}

function applyStatus(status) {
  const left = status && status.open ? Number(status.remaining_secs) || 0 : 0;
  endsAt = left > 0 ? Date.now() + left * 1000 : 0;
  if (left === 0) {
    chosenMinutes = 0;
  } else if (!chosenMinutes) {
    // Opened before this page loaded: infer the choice from what is left
    chosenMinutes = left > 600 ? 30 : 10;
  }
  render();
  clearInterval(tickTimer);
  tickTimer = left > 0 ? setInterval(onTick, 1000) : null;
}

async function onTick() {
  if (secondsLeft() > 0) {
    render();
    return;
  }
  clearInterval(tickTimer);
  tickTimer = null;
  try {
    applyStatus(await getPairingStatus());
  } catch (_err) {
    applyStatus(null);
  }
}

function renderEye(btn, open, left) {
  if (!btn) return;
  btn.classList.toggle("is-on", open);
  const icon = btn.querySelector(".pair-eye-icon");
  if (icon) icon.innerHTML = open ? EYE_ON : EYE_OFF;
  const time = btn.querySelector(".pair-eye-time");
  if (time) time.textContent = open ? formatLeft(left) : "";
  const label = open ? `Accepting pairing, ${formatLeft(left)} left` : "Pairing off";
  btn.title = label;
  btn.setAttribute("aria-label", `${label}. Change`);
}

function render() {
  const left = secondsLeft();
  const open = left > 0;

  renderEye(document.getElementById("pairEyeBtn"), open, left);
  renderEye(document.getElementById("pairEyeRailBtn"), open, left);

  const strip = document.getElementById("pairStatus");
  const text = document.getElementById("pairStatusText");
  const btn = document.getElementById("pairStatusBtn");
  if (strip) {
    strip.classList.toggle("is-off", !open);
    const icon = strip.querySelector(".pair-status-icon");
    if (icon) icon.innerHTML = open ? EYE_ON : EYE_OFF;
  }
  if (text) {
    text.innerHTML = open
      ? `Accepting pairing · <span class="pair-status-time mono">${formatLeft(left)} left</span>`
      : "Pairing is off. Other devices can't pair with this PIN or QR code.";
  }
  if (btn) {
    btn.textContent = open ? "Change" : "Turn on";
    btn.classList.toggle("btn-primary", !open);
    btn.classList.toggle("btn-ghost", open);
  }

  const dialog = document.querySelector("#pairing-modal .pairing-dialog");
  if (dialog) dialog.classList.toggle("is-pairing-off", !open);
  ["copyPinBtn", "copyLinkBtn"].forEach((id) => {
    const el = document.getElementById(id);
    if (el) el.disabled = !open;
  });

  document.querySelectorAll("#pairMenu .pair-opt").forEach((opt) => {
    const minutes = Number(opt.dataset.minutes);
    const selected = open ? minutes === chosenMinutes : minutes === 0;
    opt.classList.toggle("is-selected", selected);
    opt.setAttribute("aria-checked", String(selected));
    const check = opt.querySelector(".pair-opt-check");
    if (check) check.innerHTML = selected ? CHECK : "";
    const meta = opt.querySelector(".pair-opt-meta");
    if (meta) meta.textContent = selected && open ? `${formatLeft(left)} left` : "";
  });
}

// -------------------------------------------------------------
// Menu
// -------------------------------------------------------------
function positionMenu(menu, anchor) {
  const rect = anchor.getBoundingClientRect();
  const width = menu.offsetWidth;
  const height = menu.offsetHeight;
  const gap = 6;
  // Right-aligned to the anchor; below it when there is room (dialog), else above (sidebar)
  const left = Math.min(Math.max(8, rect.right - width), window.innerWidth - width - 8);
  const fitsBelow = rect.bottom + gap + height <= window.innerHeight - 8;
  const top = fitsBelow ? rect.bottom + gap : Math.max(8, rect.top - gap - height);
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  menu.style.transformOrigin = fitsBelow ? "top right" : "bottom right";
}

function openMenu(anchor) {
  const menu = document.getElementById("pairMenu");
  if (!menu) return;
  if (menuAnchor === anchor) {
    closeMenu();
    return;
  }
  closeMenu();
  menuAnchor = anchor;
  render();
  menu.classList.remove("hidden");
  positionMenu(menu, anchor);
  anchor.setAttribute("aria-expanded", "true");
  const current = menu.querySelector('.pair-opt[aria-checked="true"]') || menu.querySelector(".pair-opt");
  if (current) current.focus();
}

function closeMenu({ restoreFocus = false } = {}) {
  const menu = document.getElementById("pairMenu");
  if (menu) menu.classList.add("hidden");
  if (menuAnchor) {
    menuAnchor.setAttribute("aria-expanded", "false");
    if (restoreFocus) menuAnchor.focus();
  }
  menuAnchor = null;
}

async function choose(minutes) {
  closeMenu({ restoreFocus: true });
  try {
    const status = await setPairingMode(minutes);
    chosenMinutes = minutes;
    applyStatus(status);
    showToast(
      minutes ? `Accepting pairing for ${minutes} minutes` : "Pairing turned off",
      "info"
    );
  } catch (err) {
    console.error("Failed to change pairing mode:", err);
    showToast(`Could not change pairing: ${err}`, "error");
  }
}

/**
 * Wires the sidebar eye, the dialog status strip and the shared menu.
 */
export function setupPairingVisibility() {
  const menu = document.getElementById("pairMenu");
  ["pairEyeBtn", "pairEyeRailBtn", "pairStatusBtn"].forEach((id) => {
    const btn = document.getElementById(id);
    if (btn) {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        openMenu(btn);
      });
    }
  });

  if (menu) {
    menu.addEventListener("click", (e) => {
      const opt = e.target.closest(".pair-opt");
      if (opt) choose(Number(opt.dataset.minutes) || 0);
    });

    menu.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      e.preventDefault();
      const opts = [...menu.querySelectorAll(".pair-opt")];
      const idx = opts.indexOf(document.activeElement);
      const next = e.key === "ArrowDown" ? (idx + 1) % opts.length : (idx - 1 + opts.length) % opts.length;
      opts[next].focus();
    });
  }

  document.addEventListener("click", (e) => {
    if (menuAnchor && menu && !menu.contains(e.target)) closeMenu();
  });

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

  window.addEventListener("resize", () => {
    if (menuAnchor && menu) positionMenu(menu, menuAnchor);
  });

  render();
  getPairingStatus()
    .then(applyStatus)
    .catch((err) => console.error("Failed to load pairing status:", err));
}
