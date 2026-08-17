mod api;
mod bg_remove;
mod commands;
mod custom_models;
mod marketplace;
mod resource;
mod state;
mod upscale;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::command,
            commands::get_system_info,
            commands::get_app_version
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // The local scripting API port can be overridden with an env var.
            if let Ok(port) = std::env::var("RESCAYL_API_PORT") {
                if let Ok(p) = port.parse::<u16>() {
                    *handle.state::<AppState>().api_port.lock().unwrap() = p;
                }
            }

            api::start_server(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Rescayl");
}
