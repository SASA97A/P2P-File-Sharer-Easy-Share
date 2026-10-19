use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// Formats a raw byte count into a human-readable string (B, KB, MB, GB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Dispatches a native OS desktop notification if notifications are enabled in settings.
pub fn send_notification(app: &AppHandle, title: &str, body: &str) {
    if let Some(state) = app.try_state::<crate::app_state::AppState>() {
        let enabled = tauri::async_runtime::block_on(async {
            state.get_tray_settings().await.1
        });
        if !enabled {
            return;
        }
    }

    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("[Notification] Failed to display desktop notification: {}", e);
    }
}

/// Dispatches an incoming transfer request notification if the main window is hidden or unfocused.
pub fn notify_incoming_transfer(
    app: &AppHandle,
    sender_name: &str,
    file_count: usize,
    total_bytes: u64,
) {
    let should_notify = if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(true);
        let is_focused = window.is_focused().unwrap_or(true);
        !is_visible || !is_focused
    } else {
        true
    };

    if should_notify {
        let title = "Easy Share - Incoming Files";
        let count_str = if file_count == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", file_count)
        };
        let body = format!(
            "{} wants to send {} ({})",
            sender_name,
            count_str,
            format_bytes(total_bytes)
        );
        send_notification(app, title, &body);
    }
}

/// Dispatches a transfer completion notification.
pub fn notify_transfer_completed(
    app: &AppHandle,
    peer_name: &str,
    file_count: usize,
    is_received: bool,
) {
    let should_notify = if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(true);
        let is_focused = window.is_focused().unwrap_or(true);
        !is_visible || !is_focused
    } else {
        true
    };

    if should_notify {
        let count_str = if file_count == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", file_count)
        };

        if is_received {
            send_notification(
                app,
                "Easy Share - Transfer Completed",
                &format!("Successfully received {} from {}", count_str, peer_name),
            );
        } else {
            send_notification(
                app,
                "Easy Share - Files Sent",
                &format!("Successfully sent {} to {}", count_str, peer_name),
            );
        }
    }
}
