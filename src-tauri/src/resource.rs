use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// OS tag matching the `resources/{os}/bin` directory layout.
pub fn platform_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "win"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

pub fn is_win() -> bool {
    cfg!(target_os = "windows")
}

/// The name of the upscayl-ncnn executable for the current platform.
pub fn exec_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "upscayl-bin.exe"
    } else {
        "upscayl-bin"
    }
}

/// Resolves the directories containing the `upscayl-bin` executable and the
/// default models.
///
/// In packaged builds both live under the bundle's resource directory
/// (`resources/bin`, `resources/models`). In development they are read
/// straight from the repo's `resources/{os}/bin` and `resources/models`.
pub fn resolve_paths(app: &AppHandle) -> (PathBuf, PathBuf) {
    let os = platform_os();
    let mut candidates: Vec<(PathBuf, PathBuf)> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push((
            resource_dir.join("resources/bin"),
            resource_dir.join("resources/models"),
        ));
        candidates.push((resource_dir.join("bin"), resource_dir.join("models")));
        candidates.push((
            resource_dir.join("resources").join(os).join("bin"),
            resource_dir.join("resources/models"),
        ));
    }

    // Development fallback relative to the crate manifest.
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let root = PathBuf::from(manifest_dir);
        candidates.push((
            root.join("../resources").join(os).join("bin"),
            root.join("../resources/models"),
        ));
    }

    for (exec_dir, models_dir) in &candidates {
        if exec_dir.join(exec_name()).exists() {
            return (exec_dir.clone(), models_dir.clone());
        }
    }

    if let Some((exec_dir, models_dir)) = candidates.first() {
        return (exec_dir.clone(), models_dir.clone());
    }

    (PathBuf::new(), PathBuf::new())
}

/// Resolves a bundled helper script. In packaged builds it lives under
/// `resources/scripts`, in development straight in the repo's
/// `resources/scripts`.
#[allow(dead_code)]
pub fn resolve_script(app: &AppHandle, name: &str) -> Option<PathBuf> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("resources").join("scripts").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let root = PathBuf::from(manifest_dir);
        let candidate = root.join("../resources/scripts").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
