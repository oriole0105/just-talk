pub fn show(title: &str, body: &str) {
    tracing::info!(notification = true, %title, %body);
    _send(title, body);
}

pub fn show_error(title: &str, body: &str) {
    tracing::warn!(notification = true, kind = "error", %title, %body);
    _send(title, body);
}

fn _send(title: &str, body: &str) {
    // Best-effort: ignore errors (no bundle/entitlement needed for basic toasts).
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show();
}
