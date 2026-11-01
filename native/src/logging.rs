#[cfg(not(target_arch = "wasm32"))]
pub fn log(kind: &str, text: &str) {
    use std::io::Write;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("[{timestamp}] [{kind}] {text}\n");
    eprint!("{line}");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("console.txt")
    {
        let _ = f.write_all(line.as_bytes());
    }
}
#[cfg(target_arch = "wasm32")]
pub fn log(kind: &str, text: &str) {
    if matches!(kind, "GPU_ERROR" | "PANIC" | "FATAL") {
        web_sys::console::error_1(&format!("[{kind}] {text}").into());
    } else {
        web_sys::console::log_1(&format!("[{kind}] {text}").into());
    }
}
pub fn install() {
    std::panic::set_hook(Box::new(|info| log("PANIC", &info.to_string())));
    log(
        "SESSION",
        "Last World · native Bevy / GPU compute · append-only history",
    );
}
