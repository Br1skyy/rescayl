//! Local HTTP API for scripting.
//!
//! Serves `/api/v1/*` endpoints on `127.0.0.1:7860` (configurable via the
//! `RESCAYL_API_PORT` environment variable or the settings toggle). Designed
//! for headless / scripted use: submit an image as base64 or a filesystem
//! path, receive a job id, then poll `/api/v1/status/:id`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tower_http::cors::CorsLayer;

use crate::custom_models;
use crate::state::AppState;
use crate::upscale;

/// Shared server state handed to every handler.
#[derive(Clone)]
pub struct ApiShared {
    pub app: AppHandle,
    pub jobs: Arc<Mutex<HashMap<String, JobEntry>>>,
    pub job_counter: Arc<AtomicU64>,
}

/// A submitted job and its mutable status.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEntry {
    pub id: String,
    pub kind: String,
    /// One of `queued`, `running`, `done`, `error`.
    pub state: String,
    pub progress: String,
    pub message: String,
    pub result: Option<Value>,
    pub created_at: u64,
}

/// Image source for the API endpoints: either base64 data or a file path.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInput {
    /// Base64-encoded image (optionally a `data:image/...;base64,` URI).
    pub image: Option<String>,
    /// Absolute path to an image on disk.
    pub image_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleRequest {
    #[serde(flatten)]
    pub input: ImageInput,
    pub model: String,
    pub scale: Option<String>,
    pub output_format: Option<String>,
    pub tile_size: Option<i64>,
    pub tta_mode: Option<bool>,
    pub gpu_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveBgRequest {
    #[serde(flatten)]
    pub input: ImageInput,
    pub output_format: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    #[serde(flatten)]
    pub input: ImageInput,
    pub model: String,
    pub scale: Option<String>,
    pub output_format: Option<String>,
    /// Whether to remove the background before upscaling (default true).
    pub remove_background: Option<bool>,
    pub tile_size: Option<i64>,
    pub tta_mode: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobCreated {
    pub job_id: String,
    pub status: String,
    pub result: Option<Value>,
}

type ApiError = (StatusCode, Json<Value>);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Directory where API-written inputs and outputs live.
fn api_output_dir() -> PathBuf {
    let base = custom_models::default_custom_models_dir();
    let dir = base.parent().unwrap_or(&base).join("api-output");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn decode_base64_image(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let b64 = match data.split_once(',') {
        Some((prefix, rest)) if prefix.contains("base64") => rest,
        _ => data,
    };
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("Invalid base64 image: {}", e))
}

/// Resolves the input into a concrete file path, writing base64 input to a
/// temp file. Returns the path and an optional temp file to clean up.
fn resolve_input(
    _app: &AppHandle,
    input: &ImageInput,
) -> Result<(String, Option<PathBuf>), String> {
    if let Some(path) = &input.image_path {
        if path.trim().is_empty() {
            return Err("image_path is empty".into());
        }
        if !Path::new(path).is_file() {
            return Err(format!("image_path does not exist: {}", path));
        }
        return Ok((path.clone(), None));
    }
    if let Some(image) = &input.image {
        if image.trim().is_empty() {
            return Err("image is empty".into());
        }
        let bytes = decode_base64_image(image)?;
        // Write to the system temp dir, NOT the API output dir. The output
        // filename (with preserve_filename) equals the input stem + extension,
        // so a temp input sitting in the output dir would be overwritten or
        // deleted alongside the produced file.
        let dir = std::env::temp_dir().join("rescayl-api");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let name = format!("input-{}-{}.png", now_ms(), std::process::id());
        let path = dir.join(name);
        std::fs::write(&path, &bytes).map_err(|e| format!("Failed to write input image: {}", e))?;
        return Ok((path.display().to_string(), Some(path)));
    }
    Err("Provide either 'image' (base64) or 'image_path'".into())
}

fn ensure_enabled(app: &AppHandle) -> Result<(), ApiError> {
    if *app.state::<AppState>().api_enabled.lock().unwrap() {
        Ok(())
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "API is disabled. Enable it in Rescayl settings."})),
        ))
    }
}

fn bad_request(msg: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg})))
}

/// Whether `model` is a bundled model or a user-imported custom model.
fn known_model(app: &AppHandle, model: &str) -> bool {
    if upscale::known_models().contains(&model) {
        return true;
    }
    let state = app.state::<AppState>();
    if let Some(dir) = state.saved_custom_models_path.lock().unwrap().clone() {
        if custom_models::scan_flat_models(Path::new(&dir))
            .iter()
            .any(|m| m == model)
        {
            return true;
        }
    }
    custom_models::scan_custom_models(&custom_models::default_custom_models_dir())
        .iter()
        .any(|m| m.id == model)
}

fn valid_scale(scale: &str) -> bool {
    matches!(scale, "2" | "3" | "4")
}

fn normalize_format(fmt: &str) -> Result<String, String> {
    let f = fmt.to_lowercase();
    match f.as_str() {
        "png" | "jpg" | "webp" => Ok(f),
        "jpeg" => Ok("jpg".into()),
        _ => Err(format!("Unsupported output format: {}", fmt)),
    }
}

/// Builds the result payload for a completed job.
fn result_json(path: &str, format: &str) -> Value {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut value = json!({
        "outputPath": path,
        "outputFormat": format,
        "size": size,
    });
    // Attach the base64 payload when small enough to be convenient.
    if size <= 8 * 1024 * 1024 {
        use base64::Engine;
        if let Ok(bytes) = std::fs::read(path) {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            value["imageBase64"] = json!(b64);
        }
    }
    value
}

// ---- job workers ----

fn run_upscale_job(app: AppHandle, req: UpscaleRequest) -> Result<Value, String> {
    if !known_model(&app, &req.model) {
        return Err(format!("Unknown model: {}", req.model));
    }
    if let Some(scale) = &req.scale {
        if !valid_scale(scale) {
            return Err(format!("Invalid scale: {}", scale));
        }
    }
    let format = normalize_format(req.output_format.as_deref().unwrap_or("png"))?;
    let (input, temp) = resolve_input(&app, &req.input)?;
    let output_dir = api_output_dir().display().to_string();
    let payload = upscale::ImageUpscaylPayload {
        image_path: input.clone(),
        output_path: output_dir,
        scale: req.scale.clone().unwrap_or_else(|| "4".into()),
        model: req.model.clone(),
        gpu_id: req.gpu_id.clone(),
        save_image_as: format.clone(),
        overwrite: true,
        compression: "0".into(),
        no_image_processing: false,
        custom_width: None,
        use_custom_width: false,
        tile_size: req.tile_size,
        tta_mode: req.tta_mode.unwrap_or(false),
        copy_metadata: false,
        preserve_filename: true,
        bg_removal_mode: "off".into(),
        bg_removal_model: String::new(),
    };
    let outcome = upscale::process_single(&app, payload)?;
    if let Some(t) = temp {
        let _ = std::fs::remove_file(&t);
    }
    if outcome.stopped {
        return Err("Operation cancelled".into());
    }
    Ok(result_json(&outcome.output_path, &format))
}

fn run_remove_bg_job(app: AppHandle, req: RemoveBgRequest) -> Result<Value, String> {
    let format = normalize_format(req.output_format.as_deref().unwrap_or("png"))?;
    let (input, temp) = resolve_input(&app, &req.input)?;
    let stem = Path::new(&input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let out = api_output_dir().join(format!("bg-{}-{}.{}", stem, now_ms(), format));
    let out_str = out.display().to_string();
    upscale::bg_remove(&app, &input, &out_str, "isnet-general-use")?;
    if let Some(t) = temp {
        let _ = std::fs::remove_file(&t);
    }
    Ok(result_json(&out_str, &format))
}

fn run_process_job(app: AppHandle, req: ProcessRequest) -> Result<Value, String> {
    if !known_model(&app, &req.model) {
        return Err(format!("Unknown model: {}", req.model));
    }
    if let Some(scale) = &req.scale {
        if !valid_scale(scale) {
            return Err(format!("Invalid scale: {}", scale));
        }
    }
    let format = normalize_format(req.output_format.as_deref().unwrap_or("png"))?;
    let (input, temp) = resolve_input(&app, &req.input)?;

    let mut process_input = input.clone();
    let mut bg_temp: Option<PathBuf> = None;
    if req.remove_background.unwrap_or(true) {
        let stem = Path::new(&input)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image");
        let bg_out = api_output_dir().join(format!("proc-bg-{}-{}.png", stem, now_ms()));
        upscale::bg_remove(&app, &input, bg_out.to_str().unwrap(), "isnet-general-use")?;
        process_input = bg_out.display().to_string();
        bg_temp = Some(bg_out);
    }

    let output_dir = api_output_dir().display().to_string();
    let payload = upscale::ImageUpscaylPayload {
        image_path: process_input.clone(),
        output_path: output_dir,
        scale: req.scale.clone().unwrap_or_else(|| "4".into()),
        model: req.model.clone(),
        gpu_id: None,
        save_image_as: format.clone(),
        overwrite: true,
        compression: "0".into(),
        no_image_processing: false,
        custom_width: None,
        use_custom_width: false,
        tile_size: req.tile_size,
        tta_mode: req.tta_mode.unwrap_or(false),
        copy_metadata: false,
        preserve_filename: true,
        bg_removal_mode: "off".into(),
        bg_removal_model: String::new(),
    };
    let outcome = upscale::process_single(&app, payload)?;
    if let Some(t) = bg_temp {
        let _ = std::fs::remove_file(&t);
    }
    if let Some(t) = temp {
        let _ = std::fs::remove_file(&t);
    }
    if outcome.stopped {
        return Err("Operation cancelled".into());
    }
    Ok(result_json(&outcome.output_path, &format))
}

/// Queues `work` as a job and returns the job id.
fn spawn_job(
    shared: &ApiShared,
    kind: &str,
    work: impl FnOnce() -> Result<Value, String> + Send + 'static,
) -> String {
    let counter = shared.job_counter.fetch_add(1, Ordering::SeqCst);
    let id = format!("{}-{}", now_ms(), counter);
    let entry = JobEntry {
        id: id.clone(),
        kind: kind.into(),
        state: "queued".into(),
        progress: String::new(),
        message: "Queued".into(),
        result: None,
        created_at: now_ms(),
    };
    shared.jobs.lock().unwrap().insert(id.clone(), entry);
    let jobs = shared.jobs.clone();
    let id_for_thread = id.clone();
    std::thread::spawn(move || {
        {
            let mut guard = jobs.lock().unwrap();
            if let Some(e) = guard.get_mut(&id_for_thread) {
                e.state = "running".into();
                e.message = "Processing...".into();
            }
        }
        match work() {
            Ok(result) => {
                let mut guard = jobs.lock().unwrap();
                if let Some(e) = guard.get_mut(&id_for_thread) {
                    e.state = "done".into();
                    e.progress = "100".into();
                    e.message = "Completed".into();
                    e.result = Some(result);
                }
            }
            Err(err) => {
                let mut guard = jobs.lock().unwrap();
                if let Some(e) = guard.get_mut(&id_for_thread) {
                    e.state = "error".into();
                    e.message = err;
                }
            }
        }
    });
    id
}

async fn health(State(s): State<ApiShared>) -> Json<Value> {
    let state = s.app.state::<AppState>();
    let api_enabled = *state.api_enabled.lock().unwrap();
    let port = *state.api_port.lock().unwrap();
    Json(json!({
        "status": "ok",
        "service": "rescayl",
        "apiEnabled": api_enabled,
        "port": port,
        "version": s.app.package_info().version.to_string(),
    }))
}

async fn models_handler(State(s): State<ApiShared>) -> Result<Json<Value>, ApiError> {
    ensure_enabled(&s.app)?;
    let mut list: Vec<Value> = upscale::known_models()
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "name": id,
                "scale": upscale::get_model_scale(id),
                "source": "builtin",
            })
        })
        .collect();
    let custom_dir = {
        let state = s.app.state::<AppState>();
        let saved = state.saved_custom_models_path.lock().unwrap().clone();
        saved
    };
    let scan_dir = custom_dir
        .map(PathBuf::from)
        .unwrap_or_else(custom_models::default_custom_models_dir);
    for m in custom_models::scan_custom_models(&scan_dir) {
        list.push(json!({
            "id": m.id,
            "name": m.metadata.name,
            "scale": m.metadata.scale,
            "description": m.metadata.description,
            "tags": m.metadata.tags,
            "source": "custom",
        }));
    }
    Ok(Json(json!({ "models": list })))
}

async fn upscale_handler(
    State(s): State<ApiShared>,
    Json(req): Json<UpscaleRequest>,
) -> Result<Json<JobCreated>, ApiError> {
    ensure_enabled(&s.app)?;
    if req.model.trim().is_empty() {
        return Err(bad_request("Missing 'model'"));
    }
    if req.input.image.is_none() && req.input.image_path.is_none() {
        return Err(bad_request(
            "Provide either 'image' (base64) or 'image_path'",
        ));
    }
    let app = s.app.clone();
    let job_id = spawn_job(&s, "upscale", move || run_upscale_job(app, req));
    Ok(Json(JobCreated {
        job_id,
        status: "queued".into(),
        result: None,
    }))
}

async fn remove_bg_handler(
    State(s): State<ApiShared>,
    Json(req): Json<RemoveBgRequest>,
) -> Result<Json<JobCreated>, ApiError> {
    ensure_enabled(&s.app)?;
    if req.input.image.is_none() && req.input.image_path.is_none() {
        return Err(bad_request(
            "Provide either 'image' (base64) or 'image_path'",
        ));
    }
    let app = s.app.clone();
    let job_id = spawn_job(&s, "remove-bg", move || run_remove_bg_job(app, req));
    Ok(Json(JobCreated {
        job_id,
        status: "queued".into(),
        result: None,
    }))
}

async fn process_handler(
    State(s): State<ApiShared>,
    Json(req): Json<ProcessRequest>,
) -> Result<Json<JobCreated>, ApiError> {
    ensure_enabled(&s.app)?;
    if req.model.trim().is_empty() {
        return Err(bad_request("Missing 'model'"));
    }
    if req.input.image.is_none() && req.input.image_path.is_none() {
        return Err(bad_request(
            "Provide either 'image' (base64) or 'image_path'",
        ));
    }
    let app = s.app.clone();
    let job_id = spawn_job(&s, "process", move || run_process_job(app, req));
    Ok(Json(JobCreated {
        job_id,
        status: "queued".into(),
        result: None,
    }))
}

async fn status_handler(
    State(s): State<ApiShared>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobEntry>, ApiError> {
    let jobs = s.jobs.lock().unwrap();
    match jobs.get(&job_id) {
        Some(entry) => Ok(Json(entry.clone())),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Job not found"})),
        )),
    }
}

fn build_router(shared: ApiShared) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/models", get(models_handler))
        .route("/api/v1/upscale", post(upscale_handler))
        .route("/api/v1/remove-bg", post(remove_bg_handler))
        .route("/api/v1/process", post(process_handler))
        .route("/api/v1/status/{id}", get(status_handler))
        .layer(CorsLayer::permissive())
        .with_state(shared)
}

/// Starts the API server on its own tokio runtime in a background thread.
/// The server always binds so the enable/disable toggle can switch it without
/// a restart; disabled state is enforced per-request with HTTP 503.
pub fn start_server(app: AppHandle) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[rescayl-api] failed to create runtime: {}", e);
                return;
            }
        };
        rt.block_on(async move {
            let port = {
                let state = app.state::<AppState>();
                let port = *state.api_port.lock().unwrap();
                port
            };
            let shared = ApiShared {
                app: app.clone(),
                jobs: Arc::new(Mutex::new(HashMap::new())),
                job_counter: Arc::new(AtomicU64::new(0)),
            };
            let router = build_router(shared);
            let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                Ok(listener) => listener,
                Err(e) => {
                    eprintln!("[rescayl-api] failed to bind 127.0.0.1:{}: {}", port, e);
                    return;
                }
            };
            println!("[rescayl-api] listening on http://127.0.0.1:{}", port);
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("[rescayl-api] server error: {}", e);
            }
        });
    });
}
