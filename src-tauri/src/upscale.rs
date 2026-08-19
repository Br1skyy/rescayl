//! Upscaling orchestration: builds `upscayl-bin` arguments, spawns the
//! process, streams progress back to the renderer and handles the done/error
//! flow for single image, batch and double upscaling.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::resource::{exec_name, is_win, resolve_paths};
use crate::state::AppState;

// Event names match `common/electron-commands.ts`.
pub const EVENT_LOG: &str = "log";
pub const EVENT_UPSCAYL_DONE: &str = "upscayl:done";
pub const EVENT_UPSCAYL_PROGRESS: &str = "upscayl:progress";
pub const EVENT_DOUBLE_UPSCAYL_DONE: &str = "upscayl:double-done";
pub const EVENT_DOUBLE_UPSCAYL_PROGRESS: &str = "upscayl:double-progress";
pub const EVENT_FOLDER_UPSCAYL_DONE: &str = "upscayl:folder-done";
pub const EVENT_FOLDER_UPSCAYL_PROGRESS: &str = "upscayl:folder-progress";
pub const EVENT_UPSCAYL_ERROR: &str = "upscayl:error";
pub const EVENT_METADATA_ERROR: &str = "upscayl:metadata-error";
pub const EVENT_SCALING_CONVERTING: &str = "upscayl:converting";

const DEFAULT_MODELS: [&str; 10] = [
    "upscayl-standard-4x",
    "upscayl-lite-4x",
    "high-fidelity-4x",
    "remacri-4x",
    "ultramix-balanced-4x",
    "ultrasharp-4x",
    "digital-art-4x",
    "realesr-animevideov3-x2",
    "realesr-animevideov3-x3",
    "realesr-animevideov3-x4",
];

/// The names of the built-in bundled models, exposed to the API layer.
pub fn known_models() -> &'static [&'static str] {
    &DEFAULT_MODELS
}

// Payloads. Field names are camelCase on the JS side.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageUpscaylPayload {
    pub image_path: String,
    pub output_path: String,
    pub scale: String,
    pub model: String,
    pub gpu_id: Option<String>,
    pub save_image_as: String,
    pub overwrite: bool,
    pub compression: String,
    #[allow(dead_code)]
    pub no_image_processing: bool,
    pub custom_width: Option<String>,
    pub use_custom_width: bool,
    pub tile_size: Option<i64>,
    pub tta_mode: bool,
    pub copy_metadata: bool,
    #[serde(default)]
    pub preserve_filename: bool,
    #[serde(default)]
    pub bg_removal_mode: String,
    #[serde(default)]
    pub bg_removal_model: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoubleUpscaylPayload {
    pub image_path: String,
    pub output_path: String,
    pub scale: String,
    pub model: String,
    pub gpu_id: Option<String>,
    pub save_image_as: String,
    pub compression: String,
    #[allow(dead_code)]
    pub no_image_processing: bool,
    pub custom_width: Option<String>,
    pub use_custom_width: bool,
    pub tile_size: Option<i64>,
    pub tta_mode: bool,
    pub copy_metadata: bool,
    #[serde(default)]
    pub preserve_filename: bool,
    #[serde(default)]
    pub bg_removal_mode: String,
    #[serde(default)]
    pub bg_removal_model: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpscaylPayload {
    pub batch_folder_path: String,
    pub output_path: String,
    pub model: String,
    pub gpu_id: Option<String>,
    pub save_image_as: String,
    pub scale: String,
    pub compression: String,
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub no_image_processing: bool,
    pub custom_width: Option<String>,
    pub use_custom_width: bool,
    pub tile_size: Option<i64>,
    pub tta_mode: bool,
    pub copy_metadata: bool,
    #[serde(default)]
    pub bg_removal_mode: String,
    #[serde(default)]
    pub bg_removal_model: String,
}

// ---- path helpers (ports of common/* utilities) ----

fn path_sep() -> &'static str {
    if is_win() {
        "\\"
    } else {
        "/"
    }
}

fn get_file_name(p: &str) -> String {
    let sep = if p.contains('/') { '/' } else { '\\' };
    p.rsplit(sep).next().unwrap_or("").to_string()
}

fn decode_path(p: &str) -> String {
    p.to_string()
}

fn file_stem(p: &str) -> String {
    let name = get_file_name(p);
    name.rsplit_once('.')
        .map(|(s, _)| s.to_string())
        .unwrap_or(name)
}

pub(crate) fn get_model_scale(model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("x2") || m.contains("2x") {
        "2".into()
    } else if m.contains("x3") || m.contains("3x") {
        "3".into()
    } else {
        "4".into()
    }
}

#[allow(clippy::too_many_arguments)]
fn build_out_file(
    output_dir: &str,
    input_path: &str,
    scale: &str,
    custom_width: Option<&String>,
    model: &str,
    save_as: &str,
    preserve_filename: bool,
    bg_removal_mode: &str,
) -> String {
    let bg_tag = if bg_removal_mode != "off" {
        "-bgremoved"
    } else {
        ""
    };
    if preserve_filename {
        let original_name = get_file_name(input_path);
        let stem = original_name
            .rsplit_once('.')
            .map(|(s, _)| s.to_string())
            .unwrap_or(original_name);
        return format!("{}{}{}{}.{}", output_dir, path_sep(), stem, bg_tag, save_as);
    }
    let suffix = match custom_width {
        Some(w) => format!("{}px", w),
        None => format!("{}x", scale),
    };
    format!(
        "{}{}{}_upscayl_{}_{}{}.{}",
        output_dir,
        path_sep(),
        file_stem(input_path),
        suffix,
        model,
        bg_tag,
        save_as
    )
}

/// If the output path already exists, append `_2`, `_3`, etc. to make it unique.
fn dedup_out_path(out: &str) -> String {
    if !Path::new(out).exists() {
        return out.to_string();
    }
    let path = Path::new(out);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mut counter = 2u32;
    loop {
        let candidate = parent.join(format!("{}_{}.{}", stem, counter, ext));
        if !candidate.exists() {
            return candidate.display().to_string();
        }
        counter += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn build_args(
    input: String,
    out: String,
    models_dir: &Path,
    model: &str,
    scale: &str,
    custom_width: Option<String>,
    use_custom_width: bool,
    gpu_id: Option<String>,
    save_as: &str,
    compression: &str,
    tile_size: Option<i64>,
    tta_mode: bool,
) -> Vec<String> {
    let model_scale = get_model_scale(model);
    let include_scale = model_scale != scale && !use_custom_width;
    let mut args: Vec<String> = vec!["-i".into(), input, "-o".into(), out];

    if include_scale {
        args.push("-s".into());
        args.push(scale.into());
    }

    args.push("-m".into());
    args.push(models_dir.display().to_string());
    args.push("-n".into());
    args.push(model.into());

    if let Some(gpu) = gpu_id {
        if !gpu.is_empty() {
            args.push("-g".into());
            args.push(gpu);
        }
    }

    args.push("-f".into());
    args.push(save_as.into());

    if use_custom_width {
        if let Some(width) = custom_width {
            if !width.is_empty() {
                args.push("-w".into());
                args.push(width);
            }
        }
    }

    args.push("-c".into());
    args.push(compression.into());

    if let Some(tile) = tile_size {
        if tile > 0 {
            args.push("-t".into());
            args.push(tile.to_string());
        }
    }

    if tta_mode {
        args.push("-x".into());
    }

    // Tune NCNN load/proc/save threads. The binary default is 1:2:2; using
    // more proc threads speeds up tile processing on multi-core CPUs.
    if let Some(cores) = std::thread::available_parallelism().ok().map(|n| n.get()) {
        let proc = cores.clamp(2, 8);
        let save = cores.clamp(2, 4);
        args.push("-j".into());
        args.push(format!("1:{}:{}", proc, save));
    }

    args
}

fn models_dir_for(app: &AppHandle, model: &str) -> PathBuf {
    let state = app.state::<AppState>();
    let (_, default_models) = resolve_paths(app);
    if DEFAULT_MODELS.contains(&model) {
        return default_models;
    }
    let custom_models_path = state.saved_custom_models_path.lock().unwrap().clone();
    match custom_models_path {
        Some(path) => PathBuf::from(path),
        None => default_models,
    }
}

// ---- process handling ----

/// The AppImage bundler drops the exec bit from bundled resources (the
/// squashfs mount is read-only, so it cannot be fixed in place). When the
/// resolved `upscayl-bin` is not executable, copy it to the writable app
/// cache dir, mark it executable and return that path instead.
#[cfg(unix)]
fn ensure_executable(app: &AppHandle, exec: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = exec.metadata() else {
        return exec.to_path_buf();
    };
    if metadata.permissions().mode() & 0o111 != 0 {
        return exec.to_path_buf();
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("rescayl"));
    let _ = std::fs::create_dir_all(&cache_dir);
    let cached = cache_dir.join(exec_name());
    if cached
        .metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
    {
        return cached;
    }

    match std::fs::copy(exec, &cached) {
        Ok(_) => {
            let _ = std::fs::set_permissions(&cached, std::fs::Permissions::from_mode(0o755));
            cached
        }
        Err(_) => exec.to_path_buf(),
    }
}

#[cfg(not(unix))]
fn ensure_executable(_app: &AppHandle, exec: &Path) -> PathBuf {
    exec.to_path_buf()
}

/// Spawns `upscayl-bin` with the given args, resets the stop flag and
/// registers the child in shared state so STOP can kill it.
fn spawn_and_track(
    app: &AppHandle,
    args: Vec<String>,
) -> Result<(Option<ChildStdout>, Option<ChildStderr>), String> {
    let state = app.state::<AppState>();
    let (exec_dir, _) = resolve_paths(app);
    let exec = ensure_executable(app, &exec_dir.join(exec_name()));

    let filtered: Vec<&String> = args.iter().filter(|a| !a.is_empty()).collect();

    let mut cmd = Command::new(&exec);
    cmd.args(&filtered);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());

    let mut child: Child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", exec.display(), e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    *state.stopped.lock().unwrap() = false;
    let mut children = state.children.lock().unwrap();
    let mut i = 0;
    while i < children.len() {
        let exited = children[i].try_wait().map(|o| o.is_some()).unwrap_or(true);
        if exited {
            let mut dead = children.swap_remove(i);
            let _ = dead.wait();
        } else {
            i += 1;
        }
    }
    children.push(child);

    Ok((stdout, stderr))
}

fn kill_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut children = state.children.lock().unwrap();
    for child in children.iter_mut() {
        let _ = child.kill();
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let state = app.state::<AppState>();
    if *state.turn_off_notifications.lock().unwrap() {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

fn logit(app: &AppHandle, msg: &str) {
    println!("{}", msg);
    let _ = app.emit(EVENT_LOG, msg);
}

/// Streams a child's output pipe, forwarding progress events and detecting
/// failures. Returns true if the process reported an error.
fn stream_lines(app: &AppHandle, reader: impl BufRead, progress_event: &str) -> bool {
    let mut failed = false;
    for line in reader.lines() {
        let Ok(data) = line else { continue };
        if data.contains("Error") || data.contains("failed") {
            failed = true;
            let _ = app.emit(progress_event, data.clone());
            let _ = app.emit(
                EVENT_UPSCAYL_ERROR,
                format!("Error upscaling image. Error: {}", data),
            );
            kill_all(app);
        } else {
            let _ = app.emit(progress_event, data.clone());
            if data.contains("Resizing") {
                let _ = app.emit(EVENT_SCALING_CONVERTING, "");
            }
        }
    }
    failed
}

/// Spawns `upscayl-bin` and waits for it to finish, streaming progress lines
/// through `progress_event` as it runs. Returns an error if the process
/// reported a failure. The stop flag and the tracked child remain in state so
/// STOP can interrupt a running image.
fn run_process_sync(
    app: &AppHandle,
    args: Vec<String>,
    progress_event: &str,
) -> Result<(), String> {
    let (stdout, stderr) = spawn_and_track(app, args)?;
    let mut failed = false;
    if let Some(out) = stdout {
        failed |= stream_lines(app, BufReader::new(out), progress_event);
    }
    if let Some(err) = stderr {
        failed |= stream_lines(app, BufReader::new(err), progress_event);
    }
    if failed {
        return Err("upscayl-bin reported an error".into());
    }
    Ok(())
}

/// Computes the effective `-w` argument (accounting for portrait images) or
/// None when custom width is disabled.
fn effective_custom_width_arg(
    custom_width: &Option<String>,
    input: &str,
    use_custom_width: bool,
) -> Option<String> {
    if !use_custom_width {
        return None;
    }
    custom_width
        .as_ref()
        .and_then(|w| w.parse::<u32>().ok())
        .map(|w| effective_custom_width(input, w).to_string())
}

// ---- metadata ----

/// If the source image carries EXIF orientation metadata, bake it into the
/// pixel data by writing an auto-oriented temporary copy and returning its
/// path. The upscayl-bin reads raw pixels and would otherwise drop the
/// rotation, so orienting the input first keeps the output upright.
fn auto_orient(input: &str, output_dir: &str) -> Result<Option<PathBuf>, String> {
    use image::metadata::Orientation;
    use image::ImageDecoder;

    let mut reader = image::ImageReader::open(input).map_err(|e| e.to_string())?;
    reader = reader.with_guessed_format().map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;

    if orientation == Orientation::NoTransforms {
        return Ok(None);
    }

    let mut img = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);

    let stem = file_stem(input);
    let temp = PathBuf::from(output_dir).join(format!(".rescayl-orient-{}.png", stem));
    img.save(&temp).map_err(|e| e.to_string())?;
    Ok(Some(temp))
}

/// Reads the pixel dimensions of an image without decoding pixel data.
fn image_dimensions(path: &str) -> Option<(u32, u32)> {
    let mut reader = image::ImageReader::open(path).ok()?;
    reader = reader.with_guessed_format().ok()?;
    reader.into_dimensions().ok()
}

/// Computes the `-w` value so that the custom width is applied to the longer
/// side. This keeps portrait and landscape images at comparable total
/// resolutions (issue #1027): e.g. width=1600 gives 1600x1200 for landscape
/// and 1200x1600 for portrait instead of 1600x2133.
fn effective_custom_width(input: &str, custom_width: u32) -> u32 {
    if let Some((w, h)) = image_dimensions(input) {
        if w > 0 && h > w {
            // Portrait: keep the long side at custom_width.
            let ratio = w as f32 / h as f32;
            return (custom_width as f32 * ratio).round() as u32;
        }
    }
    custom_width
}

fn copy_metadata(original: &str, output: &str) -> Result<(), String> {
    let exiftool = find_exiftool()
        .ok_or_else(|| "exiftool binary not found; metadata was not copied".to_string())?;
    let status = Command::new(exiftool)
        .arg("-overwrite_original_in_place")
        .arg("-tagsFromFile")
        .arg(original)
        // Rotation is baked into the output pixels by auto_orient, so never
        // copy an EXIF Orientation tag that would rotate it a second time.
        .arg("-Orientation=1")
        .arg(output)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("exiftool failed to copy metadata".into())
    }
}

fn copy_batch_metadata(input_dir: &str, output_dir: &str) -> Result<(), String> {
    let entries = std::fs::read_dir(output_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let out_file = entry.path();
        let name = entry.file_name();
        let original = Path::new(input_dir).join(&name);
        if out_file.is_file() && original.exists() {
            copy_metadata(
                original.to_str().unwrap_or_default(),
                out_file.to_str().unwrap_or_default(),
            )?;
        }
    }
    Ok(())
}

fn find_exiftool() -> Option<PathBuf> {
    // Look for a bundled exiftool first (drop the binary into resources/).
    for candidate in [
        PathBuf::from("resources/exiftool"),
        PathBuf::from("resources/bin/exiftool"),
    ] {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for name in ["exiftool", "exiftool.exe"] {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn bg_remove(
    app: &AppHandle,
    input: &str,
    output: &str,
    _model: &str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (_, models_dir) = crate::resource::resolve_paths(app);
    let model_path = models_dir.join("bg").join("isnet-general-use.onnx");
    crate::bg_remove::remove_background(
        input,
        output,
        model_path.to_str().unwrap_or(""),
        &state.stopped,
        app,
    )
    .map_err(|e| format!("Background removal failed: {}", e))
}

/// Applies background removal "before" upscaling: writes a transparent temp
/// PNG next to the input and returns its path. The caller upscales that temp
/// file and is responsible for deleting it.
fn bg_remove_before(
    app: &AppHandle,
    input: &str,
    output_dir: &str,
    model: &str,
) -> Result<PathBuf, String> {
    let stem = file_stem(input);
    let temp = PathBuf::from(output_dir).join(format!(".rescayl-bg-{}.png", stem));
    logit(
        app,
        &format!("🎨 Removing background before upscaling ({})", model),
    );
    bg_remove(app, input, temp.to_str().unwrap(), model)?;
    Ok(temp)
}

/// Applies background removal "after" upscaling: processes the finished file
/// in place. `save_as` decides whether the result keeps transparency (png /
/// webp) or is composited onto white (jpg).
fn bg_remove_after(
    app: &AppHandle,
    output: &str,
    model: &str,
    save_as: &str,
) -> Result<(), String> {
    logit(
        app,
        &format!("🎨 Removing background after upscaling ({})", model),
    );
    let stem = file_stem(output);
    let ext = get_file_name(output)
        .rsplit_once('.')
        .map(|(_, e)| e.to_string())
        .unwrap_or_else(|| save_as.to_string());
    let temp = PathBuf::from(output)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".rescayl-bg-out-{}.{}", stem, ext));
    bg_remove(app, output, temp.to_str().unwrap(), model)?;
    std::fs::rename(&temp, output)
        .map_err(|e| format!("Failed to save background-removed image: {}", e))
}

fn bg_removal_enabled(mode: &str, model: &str) -> bool {
    !mode.is_empty() && mode != "off" && !model.is_empty()
}

// ---- public entry points ----

/// The result of processing a single image.
pub struct SingleOutcome {
    /// Path of the produced file. Empty when the run was cancelled.
    pub output_path: String,
    /// True when the user (or API) cancelled the operation part-way through.
    pub stopped: bool,
}

/// Synchronously runs the full single-image pipeline (orientation, optional
/// background removal, upscayl-bin, optional post bg removal, metadata copy)
/// and returns the path of the produced file. Progress is streamed to the
/// renderer while the process runs. Used by the batch pipeline and the local
/// HTTP API.
pub fn process_single(
    app: &AppHandle,
    payload: ImageUpscaylPayload,
) -> Result<SingleOutcome, String> {
    let output_dir = decode_path(&payload.output_path);
    let custom_width = payload.custom_width;
    let mut out_file = build_out_file(
        &output_dir,
        &payload.image_path,
        &payload.scale,
        custom_width.as_ref(),
        &payload.model,
        &payload.save_image_as,
        payload.preserve_filename,
        &payload.bg_removal_mode,
    );

    if Path::new(&out_file).exists() && !payload.overwrite {
        out_file = dedup_out_path(&out_file);
    }

    let models_dir = models_dir_for(app, &payload.model);
    let oriented = auto_orient(&payload.image_path, &output_dir)?;
    let mut input_path = oriented
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| decode_path(&payload.image_path));
    let bg_before = payload.bg_removal_mode == "before"
        && bg_removal_enabled(&payload.bg_removal_mode, &payload.bg_removal_model);
    let mut bg_temp: Option<PathBuf> = None;
    if bg_before {
        let temp = bg_remove_before(app, &input_path, &output_dir, &payload.bg_removal_model)?;
        input_path = temp.display().to_string();
        bg_temp = Some(temp);
    }

    // If user hit STOP during bg removal, bail out cleanly.
    if *app.state::<AppState>().stopped.lock().unwrap() {
        if let Some(temp) = oriented {
            let _ = std::fs::remove_file(&temp);
        }
        if let Some(temp) = bg_temp {
            let _ = std::fs::remove_file(&temp);
        }
        return Ok(SingleOutcome {
            output_path: String::new(),
            stopped: true,
        });
    }

    let effective_width =
        effective_custom_width_arg(&custom_width, &input_path, payload.use_custom_width);
    let args = build_args(
        input_path,
        out_file.clone(),
        &models_dir,
        &payload.model,
        &payload.scale,
        effective_width,
        payload.use_custom_width,
        payload.gpu_id,
        &payload.save_image_as,
        &payload.compression,
        payload.tile_size,
        payload.tta_mode,
    );
    logit(app, "🚀 Starting single image upscaling");

    let run_result = run_process_sync(app, args, EVENT_UPSCAYL_PROGRESS);

    if let Some(temp) = oriented {
        let _ = std::fs::remove_file(&temp);
    }
    if let Some(temp) = bg_temp {
        let _ = std::fs::remove_file(&temp);
    }

    run_result?;

    // If user hit STOP during upscaling, treat as cancelled.
    if *app.state::<AppState>().stopped.lock().unwrap() {
        return Ok(SingleOutcome {
            output_path: String::new(),
            stopped: true,
        });
    }

    let bg_after = payload.bg_removal_mode == "after"
        && bg_removal_enabled(&payload.bg_removal_mode, &payload.bg_removal_model);
    if bg_after {
        if let Err(e) = bg_remove_after(
            app,
            &out_file,
            &payload.bg_removal_model,
            &payload.save_image_as,
        ) {
            logit(app, &format!("❌ Error removing background: {}", e));
            return Err(e);
        }
    }
    if payload.copy_metadata {
        if let Err(e) = copy_metadata(&payload.image_path, &out_file) {
            logit(app, &format!("❌ Error copying metadata: {}", e));
            let _ = app.emit(EVENT_METADATA_ERROR, e);
        }
    }

    Ok(SingleOutcome {
        output_path: out_file,
        stopped: false,
    })
}

pub fn run_single(app: &AppHandle, payload: ImageUpscaylPayload) -> Result<(), String> {
    let app = app.clone();
    std::thread::spawn(move || match process_single(&app, payload) {
        Ok(outcome) => {
            if outcome.stopped {
                logit(&app, "⚠️ Upscaling stopped by user");
                let _ = app.emit(EVENT_UPSCAYL_DONE, "");
            } else {
                logit(&app, "💯 Done upscaling");
                let _ = app.emit(EVENT_UPSCAYL_DONE, outcome.output_path);
                notify(&app, "Rescayl", "Image upscayled successfully!");
            }
        }
        Err(e) => {
            if *app.state::<AppState>().stopped.lock().unwrap() {
                logit(&app, "⚠️ Upscaling stopped by user");
                let _ = app.emit(EVENT_UPSCAYL_DONE, "");
            } else {
                logit(&app, &format!("❌ Error upscaling image: {}", e));
                let _ = app.emit(EVENT_UPSCAYL_ERROR, e);
            }
        }
    });
    Ok(())
}

/// A single image flowing through the batch pipeline.
struct PipelineImage {
    /// The original input path (used for output naming / metadata).
    original_path: String,
    /// The file actually fed to upscayl-bin (bg-removed temp or original).
    process_path: String,
    /// Temporary bg-removed file to delete after processing, if any.
    temp_file: Option<PathBuf>,
    /// 0-based index within the batch.
    index: usize,
}

/// Aggregate counters for a batch run, shared between the pipeline threads.
#[derive(Default)]
struct BatchStats {
    succeeded: usize,
    failed: usize,
    failures: Vec<String>,
}

/// Collects the image files inside a folder (png/jpg/jpeg/webp), sorted by
/// name so batch order is stable.
fn collect_image_files(input_dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(input_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let lower = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if matches!(lower.as_str(), "png" | "jpg" | "jpeg" | "jfif" | "webp") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

pub fn run_batch(app: &AppHandle, payload: BatchUpscaylPayload) -> Result<(), String> {
    let input_dir = decode_path(&payload.batch_folder_path);
    let output_dir = decode_path(&payload.output_path);
    let custom_width = payload.custom_width;
    let suffix = match custom_width.as_ref() {
        Some(w) => format!("{}px", w),
        None => format!("{}x", payload.scale),
    };
    let folder_name = format!(
        "upscayl_{}_{}_{}",
        payload.save_image_as, payload.model, suffix
    );
    let bg_tag = if payload.bg_removal_mode != "off" {
        "-bgremoved"
    } else {
        ""
    };
    let output_folder = format!("{}{}{}{}", output_dir, path_sep(), folder_name, bg_tag);

    let bg_enabled = bg_removal_enabled(&payload.bg_removal_mode, &payload.bg_removal_model);
    let bg_before = bg_enabled && payload.bg_removal_mode == "before";
    let bg_after = bg_enabled && payload.bg_removal_mode == "after";

    let files = collect_image_files(&input_dir);
    if files.is_empty() {
        logit(app, "⚠️ No images found in the selected folder");
        let _ = app.emit(EVENT_FOLDER_UPSCAYL_DONE, output_folder);
        return Ok(());
    }
    let total = files.len();

    std::fs::create_dir_all(&output_folder).map_err(|e| e.to_string())?;
    let models_dir = models_dir_for(app, &payload.model);

    // Destructure the payload into owned values so each pipeline thread can
    // take its own copies (payload itself is not Clone).
    let app = app.clone();
    let scale = payload.scale;
    let model = payload.model.clone();
    let gpu_id = payload.gpu_id.clone();
    let save_as = payload.save_image_as.clone();
    let compression = payload.compression.clone();
    let tile_size = payload.tile_size;
    let tta_mode = payload.tta_mode;
    let use_custom_width = payload.use_custom_width;
    let copy_metadata_flag = payload.copy_metadata;
    let bg_model = payload.bg_removal_model.clone();
    let bg_model_producer = bg_model.clone();
    let bg_mode = payload.bg_removal_mode.clone();
    let custom_width = custom_width.clone();
    let input_dir = input_dir.clone();
    let output_folder_for_emit = output_folder.clone();
    let output_folder_meta = output_folder.clone();
    let models_dir = models_dir.clone();

    std::thread::spawn(move || {
        let stats = std::sync::Arc::new(std::sync::Mutex::new(BatchStats::default()));

        // Fresh batch: clear any leftover stop flag from a previous run.
        *app.state::<AppState>().stopped.lock().unwrap() = false;

        logit(
            &app,
            &format!("🚀 Starting batch upscaling ({} images)", total),
        );

        // Producer -> consumer channel with a small buffer so bg removal can
        // run ahead of the GPU-bound upscale step (pipeline parallelism).
        let (tx, rx) = std::sync::mpsc::sync_channel::<PipelineImage>(2);

        // Producer: background removal (CPU-bound via ort) for image N+1
        // while the consumer upscales image N.
        let producer = {
            let app = app.clone();
            let stats = stats.clone();
            std::thread::spawn(move || {
                for (idx, path) in files.into_iter().enumerate() {
                    if *app.state::<AppState>().stopped.lock().unwrap() {
                        break;
                    }
                    let original = path.display().to_string();
                    let mut process_path = original.clone();
                    let mut temp_file: Option<PathBuf> = None;
                    if bg_before {
                        logit(
                            &app,
                            &format!(
                                "Processing image {}/{}: removing background...",
                                idx + 1,
                                total
                            ),
                        );
                        let stem = file_stem(&original);
                        let temp = PathBuf::from(&output_dir)
                            .join(format!(".rescayl-bg-{}-{}.png", stem, idx));
                        match bg_remove(&app, &original, temp.to_str().unwrap(), &bg_model_producer)
                        {
                            Ok(()) => {
                                process_path = temp.display().to_string();
                                temp_file = Some(temp);
                            }
                            Err(e) => {
                                if *app.state::<AppState>().stopped.lock().unwrap() {
                                    break;
                                }
                                {
                                    let mut s = stats.lock().unwrap();
                                    s.failed += 1;
                                    s.failures.push(format!("{}: {}", path.display(), e));
                                }
                                logit(
                                    &app,
                                    &format!(
                                        "❌ Image {}/{} background removal failed: {}",
                                        idx + 1,
                                        total,
                                        e
                                    ),
                                );
                                continue;
                            }
                        }
                    }
                    // Drop the temp file reference if the send fails (stopped).
                    if tx
                        .send(PipelineImage {
                            original_path: original,
                            process_path,
                            temp_file,
                            index: idx,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
        };

        // Consumer: upscaling (GPU-bound via upscayl-bin), one image at a time.
        let consumer = {
            let app = app.clone();
            let stats = stats.clone();
            std::thread::spawn(move || {
                while let Ok(img) = rx.recv() {
                    if *app.state::<AppState>().stopped.lock().unwrap() {
                        if let Some(t) = &img.temp_file {
                            let _ = std::fs::remove_file(t);
                        }
                        break;
                    }
                    logit(
                        &app,
                        &format!("Processing image {}/{}: upscaling...", img.index + 1, total),
                    );
                    // Batch output naming matches upscayl-bin's folder mode:
                    // the original stem plus the target extension.
                    let out = build_out_file(
                        &output_folder,
                        &img.original_path,
                        &scale,
                        custom_width.as_ref(),
                        &model,
                        &save_as,
                        true,
                        &bg_mode,
                    );
                    let out = dedup_out_path(&out);
                    let effective_width = effective_custom_width_arg(
                        &custom_width,
                        &img.process_path,
                        use_custom_width,
                    );
                    let args = build_args(
                        img.process_path.clone(),
                        out.clone(),
                        &models_dir,
                        &model,
                        &scale,
                        effective_width,
                        use_custom_width,
                        gpu_id.clone(),
                        &save_as,
                        &compression,
                        tile_size,
                        tta_mode,
                    );
                    match run_process_sync(&app, args, EVENT_FOLDER_UPSCAYL_PROGRESS) {
                        Ok(()) => {
                            if *app.state::<AppState>().stopped.lock().unwrap() {
                                if let Some(t) = &img.temp_file {
                                    let _ = std::fs::remove_file(t);
                                }
                                break;
                            }
                            if bg_after {
                                if let Err(e) = bg_remove_after(&app, &out, &bg_model, &save_as) {
                                    let mut s = stats.lock().unwrap();
                                    s.failed += 1;
                                    s.failures.push(format!("{}: {}", img.original_path, e));
                                    drop(s);
                                    logit(
                                        &app,
                                        &format!(
                                            "❌ Image {}/{} background removal failed: {}",
                                            img.index + 1,
                                            total,
                                            e
                                        ),
                                    );
                                } else {
                                    stats.lock().unwrap().succeeded += 1;
                                }
                            } else {
                                stats.lock().unwrap().succeeded += 1;
                            }
                        }
                        Err(e) => {
                            if *app.state::<AppState>().stopped.lock().unwrap() {
                                if let Some(t) = &img.temp_file {
                                    let _ = std::fs::remove_file(t);
                                }
                                break;
                            }
                            let mut s = stats.lock().unwrap();
                            s.failed += 1;
                            s.failures.push(format!("{}: {}", img.original_path, e));
                            drop(s);
                            logit(
                                &app,
                                &format!(
                                    "❌ Image {}/{} upscaling failed: {}",
                                    img.index + 1,
                                    total,
                                    e
                                ),
                            );
                        }
                    }
                    if let Some(t) = &img.temp_file {
                        let _ = std::fs::remove_file(t);
                    }
                }
            })
        };

        producer.join().unwrap();
        consumer.join().unwrap();

        let (succeeded, failed, failures) = {
            let s = stats.lock().unwrap();
            (s.succeeded, s.failed, s.failures.clone())
        };

        if *app.state::<AppState>().stopped.lock().unwrap() {
            logit(&app, "⚠️ Batch upscaling stopped by user");
            let _ = app.emit(EVENT_FOLDER_UPSCAYL_DONE, "");
            return;
        }

        let mut meta_error = false;
        if copy_metadata_flag {
            if let Err(e) = copy_batch_metadata(&input_dir, &output_folder_meta) {
                logit(&app, &format!("❌ Error copying metadata: {}", e));
                let _ = app.emit(EVENT_METADATA_ERROR, e);
                meta_error = true;
            }
        }

        // Summary of the whole batch.
        logit(
            &app,
            &format!(
                "💯 Batch complete: {} succeeded, {} failed out of {} images.",
                succeeded, failed, total
            ),
        );
        for f in &failures {
            logit(&app, &format!("   ⚠️ {}", f));
        }
        let _ = app.emit(EVENT_FOLDER_UPSCAYL_DONE, output_folder_for_emit);
        notify(
            &app,
            "Rescayl",
            if failed > 0 || meta_error {
                "Batch upscaling finished, but some images had errors."
            } else {
                "All images upscaled successfully!"
            },
        );
    });

    Ok(())
}

pub fn run_double(app: &AppHandle, payload: DoubleUpscaylPayload) -> Result<(), String> {
    let output_dir = decode_path(&payload.output_path);
    let custom_width = payload.custom_width;
    let out_file = build_out_file(
        &output_dir,
        &payload.image_path,
        &payload.scale,
        custom_width.as_ref(),
        &payload.model,
        &payload.save_image_as,
        payload.preserve_filename,
        &payload.bg_removal_mode,
    );
    let out_file = dedup_out_path(&out_file);

    let models_dir = models_dir_for(app, &payload.model);
    let oriented = auto_orient(&payload.image_path, &output_dir)?;
    let mut input_path = oriented
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| decode_path(&payload.image_path));
    let bg_before = payload.bg_removal_mode == "before"
        && bg_removal_enabled(&payload.bg_removal_mode, &payload.bg_removal_model);
    let mut bg_temp: Option<PathBuf> = None;
    if bg_before {
        let temp = bg_remove_before(app, &input_path, &output_dir, &payload.bg_removal_model)?;
        input_path = temp.display().to_string();
        bg_temp = Some(temp);
    }

    // If user hit STOP during bg removal, bail out cleanly.
    if *app.state::<AppState>().stopped.lock().unwrap() {
        logit(app, "⚠️ Upscaling stopped by user");
        let _ = app.emit(EVENT_DOUBLE_UPSCAYL_DONE, "");
        return Ok(());
    }

    let effective_width = custom_width
        .as_ref()
        .and_then(|w| w.parse::<u32>().ok())
        .map(|w| effective_custom_width(&input_path, w).to_string());
    let pass1_args = build_args(
        input_path,
        out_file.clone(),
        &models_dir,
        &payload.model,
        &payload.scale,
        effective_width.clone(),
        payload.use_custom_width,
        payload.gpu_id.clone(),
        &payload.save_image_as,
        &payload.compression,
        payload.tile_size,
        false,
    );
    logit(app, "🚀 Starting double upscaling (pass 1)");

    let (stdout1, stderr1) = spawn_and_track(app, pass1_args)?;
    let app = app.clone();
    let oriented = oriented.clone();
    let pass2_args = build_args(
        out_file.clone(),
        out_file.clone(),
        &models_dir,
        &payload.model,
        &payload.scale,
        effective_width,
        payload.use_custom_width,
        payload.gpu_id,
        &payload.save_image_as,
        &payload.compression,
        payload.tile_size,
        payload.tta_mode,
    );
    let copy_metadata_flag = payload.copy_metadata;
    let original = payload.image_path.clone();
    let done_file = out_file.clone();
    let bg_after = payload.bg_removal_mode == "after"
        && bg_removal_enabled(&payload.bg_removal_mode, &payload.bg_removal_model);
    let bg_model = payload.bg_removal_model.clone();
    let save_as = payload.save_image_as.clone();

    std::thread::spawn(move || {
        let mut failed = false;
        if let Some(out) = stdout1 {
            failed |= stream_lines(&app, BufReader::new(out), EVENT_DOUBLE_UPSCAYL_PROGRESS);
        }
        if let Some(err) = stderr1 {
            failed |= stream_lines(&app, BufReader::new(err), EVENT_DOUBLE_UPSCAYL_PROGRESS);
        }

        let stopped = *app.state::<AppState>().stopped.lock().unwrap();
        if failed || stopped {
            if stopped {
                logit(&app, "⚠️ Upscaling stopped by user");
                let _ = app.emit(EVENT_DOUBLE_UPSCAYL_DONE, "");
            }
            if let Some(temp) = oriented {
                let _ = std::fs::remove_file(&temp);
            }
            if let Some(temp) = bg_temp {
                let _ = std::fs::remove_file(&temp);
            }
            return;
        }

        logit(&app, "🚀 Upscaling second pass");
        let (stdout2, stderr2) = match spawn_and_track(&app, pass2_args) {
            Ok(pipes) => pipes,
            Err(e) => {
                let _ = app.emit(EVENT_UPSCAYL_ERROR, e);
                return;
            }
        };

        let mut failed2 = false;
        if let Some(out) = stdout2 {
            failed2 |= stream_lines(&app, BufReader::new(out), EVENT_DOUBLE_UPSCAYL_PROGRESS);
        }
        if let Some(err) = stderr2 {
            failed2 |= stream_lines(&app, BufReader::new(err), EVENT_DOUBLE_UPSCAYL_PROGRESS);
        }

        let stopped2 = *app.state::<AppState>().stopped.lock().unwrap();
        if let Some(temp) = oriented {
            let _ = std::fs::remove_file(&temp);
        }
        if let Some(temp) = bg_temp {
            let _ = std::fs::remove_file(&temp);
        }
        if stopped2 {
            logit(&app, "⚠️ Upscaling stopped by user");
            let _ = app.emit(EVENT_DOUBLE_UPSCAYL_DONE, "");
            return;
        }
        if !failed2 {
            if bg_after {
                if let Err(e) = bg_remove_after(&app, &done_file, &bg_model, &save_as) {
                    logit(&app, &format!("❌ Error removing background: {}", e));
                    let _ = app.emit(EVENT_UPSCAYL_ERROR, e);
                    return;
                }
            }
            if copy_metadata_flag {
                if let Err(e) = copy_metadata(&original, &done_file) {
                    logit(&app, &format!("❌ Error copying metadata: {}", e));
                    let _ = app.emit(EVENT_METADATA_ERROR, e);
                }
            }
            logit(&app, "💯 Done double upscaling");
            let _ = app.emit(EVENT_DOUBLE_UPSCAYL_DONE, done_file);
            notify(&app, "Rescayl", "Image upscayled successfully!");
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inject_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
        // APP1 segment: FF E1, length(2), "Exif\0\0", TIFF(LE)
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD offset
        tiff.extend_from_slice(&1u16.to_le_bytes()); // 1 IFD entry
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // tag Orientation
        tiff.extend_from_slice(&3u16.to_le_bytes()); // type SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // count
        tiff.extend_from_slice(&(orientation as u32).to_le_bytes()); // value (padded to 4)
        tiff.extend_from_slice(&0u32.to_le_bytes()); // next IFD
        let app1_len = 6u16 + tiff.len() as u16;
        let mut app1 = Vec::new();
        app1.extend_from_slice(&[0xFF, 0xE1]);
        app1.extend_from_slice(&app1_len.to_be_bytes());
        app1.extend_from_slice(b"Exif\x00\x00");
        app1.extend_from_slice(&tiff);
        let mut out = Vec::new();
        out.extend_from_slice(&jpeg[..2]); // SOI
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn make_rotated_jpeg() -> Vec<u8> {
        let img = image::DynamicImage::new_rgb8(40, 20);
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Jpeg).unwrap();
        inject_exif_orientation(&buf.into_inner(), 6)
    }

    #[test]
    fn auto_orient_returns_none_for_plain_image() {
        let dir = std::env::temp_dir().join("rescayl-test-orient");
        std::fs::create_dir_all(&dir).unwrap();
        let img = image::DynamicImage::new_rgb8(40, 20);
        let path = dir.join("plain.png");
        img.save(&path).unwrap();
        let result = auto_orient(path.to_str().unwrap(), dir.to_str().unwrap());
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
    #[test]
    fn auto_orient_rotates_image_with_orientation_tag() {
        let dir = std::env::temp_dir().join("rescayl-test-orient");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rot.jpg");
        std::fs::write(&path, make_rotated_jpeg()).unwrap();

        let result = auto_orient(path.to_str().unwrap(), dir.to_str().unwrap());
        let temp = result.unwrap().expect("should produce oriented temp file");
        let oriented = image::ImageReader::open(&temp)
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        // Orientation=6 transposes dims: 40x20 becomes 20x40
        assert_eq!(oriented.width(), 20);
        assert_eq!(oriented.height(), 40);
        std::fs::remove_file(&temp).unwrap();
    }

    #[test]
    fn effective_custom_width_keeps_longer_side() {
        let dir = std::env::temp_dir().join("rescayl-test-width");
        std::fs::create_dir_all(&dir).unwrap();
        let landscape = dir.join("landscape.png");
        let portrait = dir.join("portrait.png");
        image::DynamicImage::new_rgb8(800, 600)
            .save(&landscape)
            .unwrap();
        image::DynamicImage::new_rgb8(600, 800)
            .save(&portrait)
            .unwrap();

        // Landscape: width stays as requested.
        assert_eq!(
            effective_custom_width(landscape.to_str().unwrap(), 1600),
            1600
        );
        // Portrait: 1600 * (600/800) = 1200, so the long side hits 1600.
        assert_eq!(
            effective_custom_width(portrait.to_str().unwrap(), 1600),
            1200
        );
    }
}
