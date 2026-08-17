//! Custom model discovery and registration.
//!
//! Scans `~/.config/rescayl/custom-models/` for user-provided NCNN models,
//! validates them, and returns their metadata to the frontend.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Optional metadata file shipped alongside a custom model.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelMetadata {
    pub name: String,
    pub description: String,
    pub scale: u32,
    pub tags: Vec<String>,
}

/// A validated custom model ready for display on the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelInfo {
    pub id: String,
    pub directory: String,
    pub metadata: CustomModelMetadata,
}

/// Returns the platform-specific default custom models directory.
///
///   Linux  : `~/.config/rescayl/custom-models`
///   macOS  : `~/Library/Application Support/org.rescayl.Rescayl/custom-models`
///   Windows: `%APPDATA%\org.rescayl.Rescayl\custom-models`
pub fn default_custom_models_dir() -> PathBuf {
    if let Some(home) = dirs() {
        if cfg!(target_os = "linux") {
            return home.join(".config").join("rescayl").join("custom-models");
        } else if cfg!(target_os = "macos") {
            return home
                .join("Library")
                .join("Application Support")
                .join("org.rescayl.Rescayl")
                .join("custom-models");
        } else if cfg!(target_os = "windows") {
            return home
                .join("AppData")
                .join("Roaming")
                .join("org.rescayl.Rescayl")
                .join("custom-models");
        }
    }
    PathBuf::from(".rescayl/custom-models")
}

fn dirs() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
}

/// Check if a directory contains a valid NCNN model (has .param and .bin files).
fn is_valid_model_dir(dir: &Path) -> bool {
    let mut has_param = false;
    let mut has_bin = false;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".param") {
                has_param = true;
            }
            if name.ends_with(".bin") {
                has_bin = true;
            }
        }
    }
    has_param && has_bin
}

/// Try to load `metadata.json` from the model directory. Returns defaults on
/// any error (missing file, parse error, etc.).
fn load_metadata(dir: &Path) -> CustomModelMetadata {
    let meta_path = dir.join("metadata.json");
    if let Ok(data) = std::fs::read_to_string(&meta_path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        // Infer scale from directory name or .param/.bin file names
        let dir_name = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let scale = infer_scale_from_name(&dir_name);
        CustomModelMetadata {
            name: dir_name.clone(),
            description: String::from("Custom model"),
            scale,
            tags: vec![],
        }
    }
}

fn infer_scale_from_name(name: &str) -> u32 {
    let lower = name.to_lowercase();
    if lower.contains("x2") || lower.contains("2x") {
        2
    } else if lower.contains("x3") || lower.contains("3x") {
        3
    } else {
        4
    }
}

/// Scan a custom models directory for valid model subdirectories.
/// Returns a list of `CustomModelInfo` structs.
pub fn scan_custom_models(dir: &Path) -> Vec<CustomModelInfo> {
    let mut models = Vec::new();

    if !dir.is_dir() {
        return models;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let subdir = entry.path();
                if is_valid_model_dir(&subdir) {
                    let mut metadata = load_metadata(&subdir);
                    let id = subdir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    // Use directory name as display name if metadata has none
                    if metadata.name.is_empty() {
                        metadata.name = id.clone();
                    }
                    models.push(CustomModelInfo {
                        id,
                        directory: subdir.display().to_string(),
                        metadata,
                    });
                }
            }
        }
    }

    models
}

/// Lists model ids (file stems) in a *flat* directory that contains
/// `.param` / `.bin` files directly (the legacy `models` folder layout).
pub fn scan_flat_models(dir: &Path) -> Vec<String> {
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

/// Ensure the default custom models directory exists.
pub fn ensure_custom_models_dir() -> PathBuf {
    let dir = default_custom_models_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}
