#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Work around blank/gray rendering on some GPUs (NVIDIA + others).
    // Must be set before the WebKit webview is created.
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    rescayl_lib::run()
}
