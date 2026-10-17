/**
 * Easy Share - Navigation Controller
 * Switches between the Send, History and Settings views and manages the
 * collapsible sidebar rail.
 */

const VIEWS = {
  send: { view: "sendView", nav: "navSendBtn" },
  history: { view: "historyView", nav: "openHistoryBtn" },
  settings: { view: "settingsView", nav: "openSettingsBtn" },
};

const NARROW_QUERY = "(max-width: 1100px)";
const NAV_STORAGE_KEY = "easyshare.navCollapsed";

let currentView = "send";

/**
 * Returns the name of the visible view
 * @returns {"send"|"history"|"settings"}
 */
export function getCurrentView() {
  return currentView;
}

/**
 * Shows one view, hides the others and updates the nav highlight
 * @param {"send"|"history"|"settings"} name
 */
export function showView(name) {
  if (!VIEWS[name]) return;
  currentView = name;

  Object.entries(VIEWS).forEach(([key, ids]) => {
    const isActive = key === name;
    const viewEl = document.getElementById(ids.view);
    const navEl = document.getElementById(ids.nav);
    if (viewEl) {
      viewEl.classList.toggle("hidden", !isActive);
    }
    if (navEl) {
      navEl.classList.toggle("is-active", isActive);
      if (isActive) {
        navEl.setAttribute("aria-current", "page");
      } else if (navEl.removeAttribute) {
        navEl.removeAttribute("aria-current");
      }
    }
  });
}

function isNarrow() {
  return typeof window !== "undefined" && window.matchMedia
    ? window.matchMedia(NARROW_QUERY).matches
    : false;
}

function readStoredCollapsed() {
  try {
    return localStorage.getItem(NAV_STORAGE_KEY) === "1";
  } catch (_err) {
    return false;
  }
}

function storeCollapsed(collapsed) {
  try {
    localStorage.setItem(NAV_STORAGE_KEY, collapsed ? "1" : "0");
  } catch (_err) {
    // Storage unavailable; the choice just won't persist
  }
}

/**
 * Initializes nav buttons and the sidebar collapse toggle
 */
export function setupNav() {
  const appBody = document.getElementById("appBody");
  const toggleBtn = document.getElementById("navToggleBtn");
  const sendNavBtn = document.getElementById("navSendBtn");

  if (sendNavBtn) {
    sendNavBtn.addEventListener("click", () => showView("send"));
  }

  if (!appBody || !toggleBtn) return;

  // On wide windows the rail is opt-in (nav-collapsed); on narrow windows it is
  // the default and the user can opt out (nav-expanded).
  const syncToggle = () => {
    const collapsed = isNarrow()
      ? !appBody.classList.contains("nav-expanded")
      : appBody.classList.contains("nav-collapsed");
    const label = collapsed ? "Expand sidebar" : "Collapse sidebar";
    toggleBtn.setAttribute("aria-label", label);
    toggleBtn.setAttribute("title", label);
    toggleBtn.setAttribute("aria-expanded", collapsed ? "false" : "true");
  };

  appBody.classList.toggle("nav-collapsed", readStoredCollapsed());

  toggleBtn.addEventListener("click", () => {
    if (isNarrow()) {
      appBody.classList.toggle("nav-expanded");
    } else {
      const collapsed = !appBody.classList.contains("nav-collapsed");
      appBody.classList.toggle("nav-collapsed", collapsed);
      storeCollapsed(collapsed);
    }
    syncToggle();
  });

  if (window.matchMedia) {
    const mq = window.matchMedia(NARROW_QUERY);
    const onChange = () => {
      appBody.classList.remove("nav-expanded");
      syncToggle();
    };
    if (mq.addEventListener) {
      mq.addEventListener("change", onChange);
    }
  }

  syncToggle();
}
