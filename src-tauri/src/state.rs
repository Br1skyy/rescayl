use std::process::Child;
use std::sync::Mutex;

use crate::custom_models::CustomModelInfo;

/// Default port for the local scripting HTTP API.
pub const DEFAULT_API_PORT: u16 = 7860;

/// Shared mutable state for the Rescayl backend.
///
/// Mirrors the variables previously held by the Electron main process:
/// the list of spawned `upscayl-bin` children (so STOP can kill them),
/// the stop flag, the persisted dialog paths, and the local API server
/// configuration.
pub struct AppState {
    pub children: Mutex<Vec<Child>>,
    pub stopped: Mutex<bool>,
    pub turn_off_notifications: Mutex<bool>,
    pub saved_image_path: Mutex<Option<String>>,
    pub saved_batch_upscayl_folder_path: Mutex<Option<String>>,
    pub saved_custom_models_path: Mutex<Option<String>>,
    pub custom_models: Mutex<Vec<CustomModelInfo>>,
    /// Whether the local scripting HTTP API accepts jobs.
    pub api_enabled: Mutex<bool>,
    /// Port the local scripting HTTP API listens on (read at startup).
    pub api_port: Mutex<u16>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            children: Mutex::new(vec![]),
            stopped: Mutex::new(false),
            turn_off_notifications: Mutex::new(false),
            saved_image_path: Mutex::new(None),
            saved_batch_upscayl_folder_path: Mutex::new(None),
            saved_custom_models_path: Mutex::new(None),
            custom_models: Mutex::new(vec![]),
            api_enabled: Mutex::new(true),
            api_port: Mutex::new(DEFAULT_API_PORT),
        }
    }
}
