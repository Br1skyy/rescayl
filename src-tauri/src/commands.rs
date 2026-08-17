//! Tauri commands. The renderer talks to the backend through the same
//! string-constant API it used with Electron (`common/electron-commands.ts`),
//! routed through the generic `command` handler below.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::custom_models;
use crate::marketplace;
use crate::resource::platform_os;
use crate::state::AppState;
use crate::upscale;

const CMD_SELECT_FILE: &str = "select-file";
const CMD_SELECT_FOLDER: &str = "select-folder";
const CMD_UPSCAYL: &str = "upscayl";
const CMD_DOUBLE_UPSCAYL: &str = "double-upscayl";
const CMD_FOLDER_UPSCAYL: &str = "folder-upscayl";
const CMD_OPEN_FOLDER: &str = "open-folder";
const CMD_SELECT_CUSTOM_MODEL_FOLDER: &str = "select-custom-model-folder";
const CMD_GET_MODELS_LIST: &str = "get-models-list";
const CMD_STOP: &str = "stop";
const CMD_PASTE_IMAGE: &str = "paste-image";
const CMD_SCAN_CUSTOM_MODELS: &str = "scan-custom-models";
const CMD_OPEN_CUSTOM_MODELS_FOLDER: &str = "open-custom-models-folder";
const CMD_FETCH_MARKETPLACE: &str = "fetch-marketplace";
const CMD_DOWNLOAD_MARKETPLACE_MODEL: &str = "download-marketplace-model";
const CMD_SAVE_MODEL_REVIEW: &str = "save-model-review";
const CMD_UNINSTALL_CUSTOM_MODEL: &str = "uninstall-custom-model";
const CMD_GET_DEFAULT_CUSTOM_MODELS_DIR: &str = "get-default-custom-models-dir";
const CMD_GET_CUSTOM_MODELS: &str = "get-custom-models";
const CMD_GET_CUSTOM_MODELS_DIR: &str = "get-custom-models-dir";
const CMD_SET_CUSTOM_MODELS_DIR: &str = "set-custom-models-dir";
const CMD_GET_API_STATUS: &str = "get-api-status";
const CMD_SET_API_ENABLED: &str = "set-api-enabled";

const EVENT_CUSTOM_MODEL_FILES_LIST: &str = "models:custom-list";
const EVENT_CUSTOM_MODELS_UPDATED: &str = "models:custom-updated";
const EVENT_PASTE_IMAGE_SAVE_SUCCESS: &str = "paste:success";
const EVENT_PASTE_IMAGE_SAVE_ERROR: &str = "paste:error";

const VALID_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "jfif", "webp"];

/// Generic dispatcher matching the old Electron IPC command strings.
///
/// `async` (not sync) so the blocking file dialogs run on the async runtime
/// thread instead of the main thread. On Linux/WebKitGTK a sync command runs
/// on the main thread, so `blocking_pick_file()` would deadlock the app.
#[tauri::command]
pub async fn command(
    app: AppHandle,
    command: String,
    payload: Option<Value>,
) -> Result<Value, String> {
    match command.as_str() {
        CMD_SELECT_FILE => select_file(&app).map(|p| json!(p)),
        CMD_SELECT_FOLDER => select_folder(&app).map(|p| json!(p)),
        CMD_UPSCAYL => {
            let p: upscale::ImageUpscaylPayload =
                serde_json::from_value(payload.unwrap_or(Value::Null))
                    .map_err(|e| e.to_string())?;
            upscale::run_single(&app, p)?;
            Ok(Value::Null)
        }
        CMD_DOUBLE_UPSCAYL => {
            let p: upscale::DoubleUpscaylPayload =
                serde_json::from_value(payload.unwrap_or(Value::Null))
                    .map_err(|e| e.to_string())?;
            upscale::run_double(&app, p)?;
            Ok(Value::Null)
        }
        CMD_FOLDER_UPSCAYL => {
            let p: upscale::BatchUpscaylPayload =
                serde_json::from_value(payload.unwrap_or(Value::Null))
                    .map_err(|e| e.to_string())?;
            upscale::run_batch(&app, p)?;
            Ok(Value::Null)
        }
        CMD_OPEN_FOLDER => {
            let path = payload
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            open_folder(path).map(|_| Value::Null)
        }
        CMD_SELECT_CUSTOM_MODEL_FOLDER => {
            select_custom_model_folder(&app).map(|v| json!(v))
        }
        CMD_GET_MODELS_LIST => {
            let path = payload.and_then(|v| v.as_str().map(|s| s.to_string()));
            get_models_list(&app, path);
            Ok(Value::Null)
        }
        CMD_STOP => {
            stop(&app);
            Ok(Value::Null)
        }
        CMD_PASTE_IMAGE => {
            paste_image(&app, payload).map(|_| Value::Null)
        }
        CMD_SCAN_CUSTOM_MODELS => {
            scan_custom_models_cmd(&app);
            Ok(Value::Null)
        }
        CMD_OPEN_CUSTOM_MODELS_FOLDER => {
            open_custom_models_folder(&app);
            Ok(Value::Null)
        }
        CMD_FETCH_MARKETPLACE => {
            fetch_marketplace_cmd(&app).await
        }
        CMD_DOWNLOAD_MARKETPLACE_MODEL => {
            download_marketplace_model_cmd(&app, payload).await
        }
        CMD_SAVE_MODEL_REVIEW => {
            save_model_review_cmd(payload).map(|_| Value::Null)
        }
        CMD_UNINSTALL_CUSTOM_MODEL => {
            uninstall_custom_model_cmd(&app, payload)?;
            Ok(Value::Null)
        }
        CMD_GET_DEFAULT_CUSTOM_MODELS_DIR => {
            Ok(json!(custom_models::default_custom_models_dir().display().to_string()))
        }
        CMD_GET_CUSTOM_MODELS => Ok(json!(get_custom_models(&app))),
        CMD_GET_CUSTOM_MODELS_DIR => Ok(json!(current_custom_models_dir(&app).display().to_string())),
        CMD_SET_CUSTOM_MODELS_DIR => {
            let path = payload
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            set_custom_models_dir(&app, path);
            Ok(Value::Null)
        }
        CMD_GET_API_STATUS => Ok(json!({
            "enabled": *app.state::<AppState>().api_enabled.lock().unwrap(),
            "port": *app.state::<AppState>().api_port.lock().unwrap(),
        })),
        CMD_SET_API_ENABLED => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ApiToggle {
                enabled: Option<bool>,
                port: Option<u16>,
            }
            let p: ApiToggle = serde_json::from_value(payload.unwrap_or(Value::Null))
                .map_err(|e| e.to_string())?;
            if let Some(enabled) = p.enabled {
                *app.state::<AppState>().api_enabled.lock().unwrap() = enabled;
            }
            if let Some(port) = p.port {
                if port > 0 {
                    *app.state::<AppState>().api_port.lock().unwrap() = port;
                }
            }
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown command: {}", command)),
    }
}

/// Returns the active custom models directory: the saved one if set,
/// otherwise the platform default (`~/.config/rescayl/custom-models`).
fn current_custom_models_dir(app: &AppHandle) -> PathBuf {
    let state = app.state::<AppState>();
    let saved = state.saved_custom_models_path.lock().unwrap().clone();
    match saved {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => custom_models::default_custom_models_dir(),
    }
}

fn set_custom_models_dir(app: &AppHandle, path: String) {
    let state = app.state::<AppState>();
    *state.saved_custom_models_path.lock().unwrap() = Some(path);
}

fn get_custom_models(app: &AppHandle) -> Vec<custom_models::CustomModelInfo> {
    let state = app.state::<AppState>();
    let dir = current_custom_models_dir(app);
    let models = custom_models::scan_custom_models(&dir);
    *state.custom_models.lock().unwrap() = models.clone();
    models
}

fn scan_custom_models_cmd(app: &AppHandle) {
    let state = app.state::<AppState>();
    let dir = current_custom_models_dir(app);
    let models = custom_models::scan_custom_models(&dir);
    *state.custom_models.lock().unwrap() = models.clone();
    let ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
    let _ = app.emit(EVENT_CUSTOM_MODEL_FILES_LIST, ids);
    let _ = app.emit(EVENT_CUSTOM_MODELS_UPDATED, models);
}

fn open_custom_models_folder(app: &AppHandle) {
    let dir = current_custom_models_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    let _ = opener::open(&dir);
}

async fn fetch_marketplace_cmd(app: &AppHandle) -> Result<Value, String> {
    let manifest = marketplace::fetch_manifest(app).await?;
    Ok(serde_json::to_value(manifest).map_err(|e| e.to_string())?)
}

async fn download_marketplace_model_cmd(app: &AppHandle, payload: Option<Value>) -> Result<Value, String> {
    let model_id = payload
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| "Missing model id".to_string())?;
    let manifest = marketplace::cached_manifest();
    let model = manifest
        .models
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Model '{}' not found in marketplace", model_id))?;
    let custom_models_dir = current_custom_models_dir(app);
    let _ = std::fs::create_dir_all(&custom_models_dir);
    let dir = match marketplace::download_model(app, &model, &custom_models_dir).await {
        Ok(dir) => dir,
        Err(e) => {
            // Clean up any partially-downloaded model directory so a failed
            // install does not leave an empty phantom model behind.
            let _ = std::fs::remove_dir_all(custom_models_dir.join(&model_id));
            return Err(e);
        }
    };
    // Refresh the custom models list immediately so an installed model shows
    // up without the user having to hit Rescan.
    scan_custom_models_cmd(app);
    Ok(json!({
        "installed": true,
        "id": model_id,
        "directory": dir,
    }))
}

/// Uninstall a custom model by id (removes its directory from the custom
/// models folder). Refuses to touch anything outside the custom models dir.
fn uninstall_custom_model_cmd(app: &AppHandle, payload: Option<Value>) -> Result<(), String> {
    let model_id = payload
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| "Missing model id".to_string())?;
    let custom_models_dir = current_custom_models_dir(app);
    let dir = custom_models_dir.join(&model_id);

    if !dir.exists() {
        return Err(format!("Model '{}' is not installed", model_id));
    }
    if !dir.is_dir() {
        return Err(format!("'{}' is not a model directory", model_id));
    }

    let base = std::fs::canonicalize(&custom_models_dir)
        .unwrap_or_else(|_| custom_models_dir.clone());
    let target = std::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
    if !target.starts_with(&base) {
        return Err(
            "Refusing to delete a path outside the custom models directory".to_string(),
        );
    }

    std::fs::remove_dir_all(&target)
        .map_err(|e| format!("Failed to uninstall model '{}': {}", model_id, e))?;
    scan_custom_models_cmd(app);
    Ok(())
}

fn save_model_review_cmd(payload: Option<Value>) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Review {
        model_id: String,
        rating: u8,
        review: String,
    }
    let review: Review = serde_json::from_value(payload.unwrap_or(Value::Null))
        .map_err(|e| e.to_string())?;
    marketplace::save_review(&review.model_id, review.rating, &review.review)?;
    Ok(Value::Null)
}

fn select_file(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let default_dir = state.saved_image_path.lock().unwrap().clone();

    let mut picker = app.dialog().file().add_filter(
        "Images",
        &["png", "jpg", "jpeg", "jfif", "webp", "PNG", "JPG", "JPEG", "JFIF", "WEBP"],
    );
    if let Some(dir) = default_dir {
        picker = picker.set_directory(dir);
    }

    let file = picker.blocking_pick_file();
    let Some(file) = file else {
        return Ok(None);
    };
    let Ok(path) = file.into_path() else {
        return Ok(None);
    };

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !VALID_EXTENSIONS.contains(&ext.as_str()) {
        app.dialog()
            .message("The selected file is not a valid image. Make sure you select a '.png', '.jpg', or '.webp' file.")
            .title("Invalid File")
            .blocking_show();
        return Ok(None);
    }

    *state.saved_image_path.lock().unwrap() = Some(path.display().to_string());
    Ok(Some(path.display().to_string()))
}

fn select_folder(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let default_dir = state
        .saved_batch_upscayl_folder_path
        .lock()
        .unwrap()
        .clone();

    let mut picker = app.dialog().file();
    if let Some(dir) = default_dir {
        picker = picker.set_directory(dir);
    }

    let folder = picker.blocking_pick_folder();
    let Some(folder) = folder else {
        return Ok(None);
    };
    let Ok(path) = folder.into_path() else {
        return Ok(None);
    };

    *state.saved_batch_upscayl_folder_path.lock().unwrap() = Some(path.display().to_string());
    Ok(Some(path.display().to_string()))
}

fn open_folder(path: String) -> Result<(), String> {
    opener::open(path).map_err(|e| e.to_string())
}

fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.stopped.lock().unwrap() = true;
    let mut children = state.children.lock().unwrap();
    for child in children.iter_mut() {
        let _ = child.kill();
    }
}

fn scan_models(dir: &Path) -> Vec<String> {
    let mut models: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.ends_with(".param") || lower.ends_with(".bin") {
                let stem = name
                    .rsplit_once('.')
                    .map(|(s, _)| s.to_string())
                    .unwrap_or(name.clone());
                if !models.contains(&stem) {
                    models.push(stem);
                }
            }
        }
    }
    models
}

fn select_custom_model_folder(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let default_dir = state.saved_custom_models_path.lock().unwrap().clone();

    let mut picker = app.dialog().file();
    if let Some(dir) = default_dir {
        picker = picker.set_directory(dir);
    }

    let folder = picker.blocking_pick_folder();
    let Some(folder) = folder else {
        return Ok(None);
    };
    let Ok(path) = folder.into_path() else {
        return Ok(None);
    };
    let path_str = path.display().to_string();

    let sep = if platform_os() == "win" { "\\" } else { "/" };
    if !path_str.ends_with(&format!("{}models", sep))
        && !path_str.ends_with(&format!("{}models{}", sep, sep))
    {
        app.dialog()
            .message("Please make sure that the folder name is 'models' and nothing else.")
            .title("Invalid Folder")
            .blocking_show();
        return Ok(None);
    }

    let models = scan_models(&path);
    if models.is_empty() {
        app.dialog()
            .message("The selected folder does not contain valid model files. Make sure you select the folder that ONLY contains '.param' and '.bin' files.")
            .title("Invalid Folder")
            .blocking_show();
        return Ok(None);
    }

    *state.saved_custom_models_path.lock().unwrap() = Some(path_str.clone());
    let _ = app.emit(EVENT_CUSTOM_MODEL_FILES_LIST, models);
    Ok(Some(path_str))
}

fn get_models_list(app: &AppHandle, path: Option<String>) {
    let Some(path) = path else { return };
    let state = app.state::<AppState>();
    *state.saved_custom_models_path.lock().unwrap() = Some(path.clone());
    let models = scan_models(Path::new(&path));
    let _ = app.emit(EVENT_CUSTOM_MODEL_FILES_LIST, models);
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct PasteImagePayload {
    name: String,
    path: String,
    extension: String,
    size: Option<u64>,
    #[serde(rename = "type")]
    file_type: Option<String>,
    #[serde(rename = "encodedBuffer")]
    encoded_buffer: String,
}

fn paste_image(app: &AppHandle, payload: Option<Value>) -> Result<(), String> {
    let p: PasteImagePayload = serde_json::from_value(payload.unwrap_or(Value::Null))
        .map_err(|e| e.to_string())?;

    let ext = p.extension.to_lowercase();
    if !VALID_EXTENSIONS.contains(&ext.as_str()) {
        let _ = app.emit(EVENT_PASTE_IMAGE_SAVE_ERROR, "Unsupported Image Format");
        return Ok(());
    }

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(p.encoded_buffer)
        .map_err(|e| format!("Failed to decode pasted image: {}", e))?;

    let file_path = PathBuf::from(&p.path).join(&p.name);
    std::fs::write(&file_path, bytes).map_err(|e| e.to_string())?;
    let _ = app.emit(EVENT_PASTE_IMAGE_SAVE_SUCCESS, file_path.display().to_string());
    Ok(())
}

/// Returns a best-effort description of the machine (CPU etc.). GPU
/// enumeration is not available on every platform, so `gpu` may be omitted.
#[tauri::command]
pub fn get_system_info() -> Value {
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());

    let cpus = sys.cpus();
    let model = cpus.first().map(|c| c.brand().to_string()).unwrap_or_default();

    json!({
        "platform": platform_os(),
        "release": sysinfo::System::kernel_version().unwrap_or_default(),
        "arch": std::env::consts::ARCH,
        "model": model,
        "cpuCount": cpus.len(),
    })
}

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}
