/**
 * Lucide Icon Library & SVG Renderer for Easy Share
 * High-quality, consistent stroke-based SVG icons
 */

const LUCIDE_ATTRS = 'xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"';

export const ICONS = {
  monitor: `<svg ${LUCIDE_ATTRS} class="lucide lucide-monitor"><rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/></svg>`,
  laptop: `<svg ${LUCIDE_ATTRS} class="lucide lucide-laptop"><path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/></svg>`,
  smartphone: `<svg ${LUCIDE_ATTRS} class="lucide lucide-smartphone"><rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/></svg>`,
  check: `<svg ${LUCIDE_ATTRS} class="lucide lucide-check"><path d="M20 6 9 17l-5-5"/></svg>`,
  checkCircle: `<svg ${LUCIDE_ATTRS} class="lucide lucide-check-circle"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><path d="m9 11 3 3L22 4"/></svg>`,
  x: `<svg ${LUCIDE_ATTRS} class="lucide lucide-x"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>`,
  alertCircle: `<svg ${LUCIDE_ATTRS} class="lucide lucide-alert-circle"><circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/></svg>`,
  info: `<svg ${LUCIDE_ATTRS} class="lucide lucide-info"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>`,
  fileText: `<svg ${LUCIDE_ATTRS} class="lucide lucide-file-text"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/></svg>`,
  plusCircle: `<svg ${LUCIDE_ATTRS} class="lucide lucide-plus-circle"><circle cx="12" cy="12" r="10"/><path d="M8 12h8"/><path d="M12 8v8"/></svg>`,
  uploadCloud: `<svg ${LUCIDE_ATTRS} class="lucide lucide-upload-cloud"><path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242"/><path d="M12 12v9"/><path d="m16 16-4-4-4 4"/></svg>`,
  send: `<svg ${LUCIDE_ATTRS} class="lucide lucide-send"><path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/></svg>`,
  shieldCheck: `<svg ${LUCIDE_ATTRS} class="lucide lucide-shield-check"><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/></svg>`,
  hardDriveDownload: `<svg ${LUCIDE_ATTRS} class="lucide lucide-hard-drive-download"><path d="M12 2v8"/><path d="m16 6-4 4-4-4"/><rect width="20" height="8" x="2" y="14" rx="2"/><path d="M6 18h.01"/><path d="M10 18h.01"/></svg>`,
  qrCode: `<svg ${LUCIDE_ATTRS} class="lucide lucide-qr-code"><rect width="5" height="5" x="3" y="3" rx="1"/><rect width="5" height="5" x="16" y="3" rx="1"/><rect width="5" height="5" x="3" y="16" rx="1"/><path d="M21 16h-3a2 2 0 0 0-2 2v3"/><path d="M21 21v.01"/><path d="M12 7v3a2 2 0 0 1-2 2H7"/><path d="M3 12h.01"/><path d="M12 3h.01"/><path d="M12 16v.01"/><path d="M16 12h1"/><path d="M21 12v.01"/><path d="M12 21v-1"/></svg>`,
  refreshCw: `<svg ${LUCIDE_ATTRS} class="lucide lucide-refresh-cw"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>`,
  copy: `<svg ${LUCIDE_ATTRS} class="lucide lucide-copy"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`,
  hash: `<svg ${LUCIDE_ATTRS} class="lucide lucide-hash"><line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/></svg>`,
  arrowRight: `<svg ${LUCIDE_ATTRS} class="lucide lucide-arrow-right"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>`,
  wifi: `<svg ${LUCIDE_ATTRS} class="lucide lucide-wifi"><path d="M12 20h.01"/><path d="M2 8.82a15 15 0 0 1 20 0"/><path d="M5 12.859a10 10 0 0 1 14 0"/><path d="M8.5 16.429a5 5 0 0 1 7 0"/></svg>`,
  link: `<svg ${LUCIDE_ATTRS} class="lucide lucide-link"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>`,
  loader2: `<svg ${LUCIDE_ATTRS} class="lucide lucide-loader-2"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>`
};

/**
 * Returns the SVG string for a given Lucide icon name
 * @param {keyof typeof ICONS} name
 * @param {Object} [options]
 * @param {number} [options.size=24]
 * @param {string} [options.class=""]
 * @returns {string}
 */
export function getIconSvg(name, options = {}) {
  const raw = ICONS[name] || ICONS.fileText;
  const size = options.size || 24;
  const customClass = options.class ? ` ${options.class}` : "";

  return raw.replace('<svg ', `<svg width="${size}" height="${size}" `).replace('class="', `class="${customClass} `);
}

/**
 * Resolves the appropriate Lucide icon for a device based on OS and type
 * @param {string} os
 * @param {string} deviceType
 * @param {Object} [options]
 * @returns {string} SVG HTML string
 */
export function getDeviceIconSvg(os = "", deviceType = "", options = {}) {
  const osLower = (os || "").toLowerCase();
  const typeLower = (deviceType || "").toLowerCase();

  if (osLower.includes("darwin") || osLower.includes("mac") || osLower.includes("ios")) {
    return getIconSvg("laptop", options);
  }
  if (
    typeLower.includes("mobile") ||
    typeLower.includes("phone") ||
    osLower.includes("android")
  ) {
    return getIconSvg("smartphone", options);
  }
  return getIconSvg("monitor", options);
}
