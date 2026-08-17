//! Model marketplace: fetch a remote manifest, download models, and
//! install them into the custom models directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

// Event names for frontend progress updates.
pub const EVENT_MARKETPLACE_PROGRESS: &str = "marketplace:download-progress";
pub const EVENT_MARKETPLACE_DONE: &str = "marketplace:download-done";
pub const EVENT_MARKETPLACE_ERROR: &str = "marketplace:download-error";

/// A single model entry in the marketplace manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceModel {
    pub id: String,
    pub name: String,
    pub author: String,
    pub scale: u32,
    pub description: String,
    pub size: String,
    pub download_url: String,
    pub preview_url: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub rating: f32,
    /// URL to the .param file (for dual-file NCNN models).
    #[serde(default)]
    pub param_url: Option<String>,
    /// URL to the .bin file (for dual-file NCNN models).
    #[serde(default)]
    pub bin_url: Option<String>,
}

/// Top-level marketplace manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceManifest {
    pub models: Vec<MarketplaceModel>,
}

/// Local user review data stored alongside the marketplace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelReview {
    pub model_id: String,
    pub rating: u8,
    pub review: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserReviews {
    pub reviews: Vec<ModelReview>,
}

/// Path to the locally-cached marketplace manifest.
fn manifest_cache_path() -> PathBuf {
    let base = crate::custom_models::ensure_custom_models_dir();
    base.parent()
        .unwrap_or(&base)
        .join("marketplace-manifest.json")
}

/// Path to user reviews.
fn reviews_path() -> PathBuf {
    let base = crate::custom_models::ensure_custom_models_dir();
    base.parent()
        .unwrap_or(&base)
        .join("marketplace-reviews.json")
}

/// Resolves the bundled marketplace manifest (`resources/marketplace.json`)
/// across packaged and development layouts. Covers:
///   - packaged: `<resource_dir>/resources/marketplace.json`
///   - `cargo run` from src-tauri: `<crate>/../resources/marketplace.json`
///   - binary launched directly from the repo root: `<cwd>/resources/marketplace.json`
///   - binary launched from elsewhere: `<exe>/resources` and `<exe>/../resources`
pub fn bundled_manifest_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("resources/marketplace.json"));
        candidates.push(resource_dir.join("marketplace.json"));
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest_dir).join("../resources/marketplace.json"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("resources/marketplace.json"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources/marketplace.json"));
            candidates.push(exe_dir.join("../resources/marketplace.json"));
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Load the marketplace manifest. The bundled `resources/marketplace.json`
/// is authoritative; it ships with the app and is bundled by tauri.conf.json.
pub async fn fetch_manifest(app: &AppHandle) -> Result<MarketplaceManifest, String> {
    if let Some(path) = bundled_manifest_path(app) {
        let data =
            std::fs::read_to_string(&path).map_err(|e| format!("Failed to read marketplace manifest at {}: {}", path.display(), e))?;
        let manifest = serde_json::from_str::<MarketplaceManifest>(&data)
            .map_err(|e| format!("Invalid marketplace manifest: {}", e))?;
        let _ = std::fs::write(manifest_cache_path(), &data);
        return Ok(manifest);
    }
    Err("Marketplace manifest not found (resources/marketplace.json is missing)".into())
}

/// Return cached manifest (or empty if not cached).
pub fn cached_manifest() -> MarketplaceManifest {
    let path = manifest_cache_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or(MarketplaceManifest { models: vec![] })
    } else {
        MarketplaceManifest { models: vec![] }
    }
}

/// Download a model's .param and .bin files into the custom models directory.
///
/// For dual-file NCNN models (param_url + bin_url), both files are downloaded
/// into a subdirectory named after the model id.
pub async fn download_model(
    app: &AppHandle,
    model: &MarketplaceModel,
    custom_models_dir: &Path,
) -> Result<String, String> {
    let model_dir = custom_models_dir.join(&model.id);
    std::fs::create_dir_all(&model_dir)
        .map_err(|e| format!("Failed to create model directory: {}", e))?;

    // If dual-URL model (param_url + bin_url), download both files
    if let (Some(param_url), Some(bin_url)) = (&model.param_url, &model.bin_url) {
        let param_file = model_dir.join(format!("{}.param", model.id));
        download_file(app, param_url, &param_file, &model.id, "param").await?;

        let bin_file = model_dir.join(format!("{}.bin", model.id));
        download_file(app, bin_url, &bin_file, &model.id, "bin").await?;
    } else if !model.download_url.is_empty() {
        // Single download URL — treat as param URL; also look for a companion bin
        let param_file = model_dir.join(format!("{}.param", model.id));
        download_file(app, &model.download_url, &param_file, &model.id, "param").await?;

        // Try to download a .bin companion (same base URL with .bin extension)
        let bin_url = model.download_url.replace(".param", ".bin");
        let bin_file = model_dir.join(format!("{}.bin", model.id));
        if bin_url != model.download_url {
            let _ = download_file(app, &bin_url, &bin_file, &model.id, "bin").await;
        }
    }

    // Write metadata.json
    let meta = crate::custom_models::CustomModelMetadata {
        name: model.name.clone(),
        description: model.description.clone(),
        scale: model.scale,
        tags: model.tags.clone(),
    };
    let meta_path = model_dir.join("metadata.json");
    let _ = std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    );

    let _ = app.emit(EVENT_MARKETPLACE_DONE, &model.id);
    Ok(model_dir.display().to_string())
}

/// Download a single file with progress reporting. Streams the response to
/// disk and emits `marketplace:download-progress` events as bytes arrive.
/// On failure, emits `marketplace:download-error` and returns the error.
async fn download_file(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    model_id: &str,
    file_label: &str,
) -> Result<(), String> {
    let result = download_file_inner(app, url, dest, model_id, file_label).await;
    if let Err(ref e) = result {
        let _ = app.emit(
            EVENT_MARKETPLACE_ERROR,
            serde_json::json!({
                "modelId": model_id,
                "file": file_label,
                "error": e,
            }),
        );
    }
    result
}

async fn download_file_inner(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    model_id: &str,
    file_label: &str,
) -> Result<(), String> {
    use futures_util::StreamExt;

    // Retry transient failures (HTTP 429 rate-limits, 5xx, network blips)
    // with a short backoff before giving up.
    let resp = {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match reqwest::get(url).await {
                Ok(r) if r.status().is_success() => break r,
                Ok(r) => {
                    if attempt < 3
                        && (r.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                            || r.status().is_server_error())
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(1200 * attempt)).await;
                        continue;
                    }
                    return Err(format!(
                        "Failed to download {}: HTTP {}",
                        file_label,
                        r.status()
                    ));
                }
                Err(e) => {
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_millis(1200 * attempt)).await;
                        continue;
                    }
                    return Err(format!("Failed to download {}: {}", file_label, e));
                }
            }
        }
    };

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Failed to create {}: {}", file_label, e))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        use tokio::io::AsyncWriteExt;
        let chunk = chunk.map_err(|e| format!("Failed to read {}: {}", file_label, e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write {}: {}", file_label, e))?;
        downloaded += chunk.len() as u64;
        let progress = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0) as u8
        } else {
            0
        };
        let _ = app.emit(
            EVENT_MARKETPLACE_PROGRESS,
            serde_json::json!({
                "modelId": model_id,
                "file": file_label,
                "progress": progress,
                "downloaded": downloaded,
                "total": total,
            }),
        );
    }

    {
        use tokio::io::AsyncWriteExt;
        file.flush()
            .await
            .map_err(|e| format!("Failed to write {}: {}", file_label, e))?;
    }

    let _ = app.emit(
        EVENT_MARKETPLACE_PROGRESS,
        serde_json::json!({
            "modelId": model_id,
            "file": file_label,
            "progress": 100,
        }),
    );

    Ok(())
}

/// Save a user review for a model.
pub fn save_review(model_id: &str, rating: u8, review: &str) -> Result<(), String> {
    let mut reviews = load_reviews();
    // Remove existing review for this model
    reviews.retain(|r| r.model_id != model_id);
    reviews.push(ModelReview {
        model_id: model_id.to_string(),
        rating,
        review: review.to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });
    let path = reviews_path();
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&UserReviews { reviews })
            .unwrap_or_default(),
    )
    .map_err(|e| format!("Failed to save review: {}", e))
}

/// Load user reviews.
pub fn load_reviews() -> Vec<ModelReview> {
    let path = reviews_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(reviews) = serde_json::from_str::<UserReviews>(&data) {
            return reviews.reviews;
        }
    }
    vec![]
}
