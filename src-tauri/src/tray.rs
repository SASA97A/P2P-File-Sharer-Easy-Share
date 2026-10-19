use std::path::Path;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub const TRAY_ID: &str = "main_tray";

/// Formats a dynamic tray tooltip string based on transfer direction, percent, speed, and active count.
pub fn format_transfer_tooltip(
    direction: &str,
    percent: u8,
    speed_bps: Option<u64>,
    file_name: Option<&str>,
) -> String {
    let speed_part = match speed_bps {
        Some(bps) if bps > 0 => {
            if bps >= 1_048_576 {
                let mb_s = (bps as f64) / (1024.0 * 1024.0);
                format!(" · {:.1} MB/s", mb_s)
            } else {
                format!(" · {} KB/s", bps / 1024)
            }
        }
        _ => String::new(),
    };

    let target_part = match file_name {
        Some(name) if !name.is_empty() => format!(" ({})", name),
        _ => String::new(),
    };

    format!(
        "Easy Share - {} ({}%{}){}",
        direction, percent, speed_part, target_part
    )
}

/// Sets up the cross-platform system tray icon, context menu, and click events.
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::with_id("show", "Show Easy Share").build(app)?;
    let downloads_item = MenuItemBuilder::with_id("open_downloads", "Open Downloads").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit Easy Share").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&downloads_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Easy Share - Ready")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"));

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .on_menu_event(move |app_handle, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "open_downloads" => {
                if let Some(state) = app_handle.try_state::<crate::app_state::AppState>() {
                    let dl_dir = state.get_download_dir();
                    let _ = open_directory(&dl_dir);
                }
            }
            "quit" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            #[cfg(not(target_os = "macos"))]
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if let Ok(visible) = window.is_visible() {
                        if visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Dynamically updates the system tray tooltip (e.g. during active transfers).
pub fn update_tray_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

/// Opens the target directory in the platform's native file explorer.
fn open_directory(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open finder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    Ok(())
}
