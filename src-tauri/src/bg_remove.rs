use std::path::Path;
use std::sync::Mutex;

use image::DynamicImage;
use ndarray::Array;
use ort::session::Session;
use tauri::{AppHandle, Emitter};

const MODEL_SIZE: u32 = 1024;

pub fn remove_background(
    input_path: &str,
    output_path: &str,
    model_path: &str,
    stopped: &Mutex<bool>,
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    if *stopped.lock().unwrap() {
        return Err("Cancelled".into());
    }

    if !Path::new(model_path).exists() {
        return Err(format!(
            "Background removal model not found at '{}'. \
             Download isnet-general-use.onnx into resources/models/bg/.",
            model_path
        )
        .into());
    }

    let _ = app.emit("log", "Loading background removal model...");
    let mut session = Session::builder()?.commit_from_file(model_path)?;

    if *stopped.lock().unwrap() {
        return Err("Cancelled".into());
    }

    let _ = app.emit("log", "Reading input image...");
    let img = image::ImageReader::open(input_path)?
        .with_guessed_format()?
        .decode()?;
    let original = img.to_rgba8();
    let (orig_w, orig_h) = original.dimensions();

    if *stopped.lock().unwrap() {
        return Err("Cancelled".into());
    }

    let _ = app.emit("log", "Preprocessing image for background removal...");
    let resized = image::imageops::resize(
        &original,
        MODEL_SIZE,
        MODEL_SIZE,
        image::imageops::FilterType::Lanczos3,
    );

    let mut input_tensor = Array::zeros((1, 3, MODEL_SIZE as usize, MODEL_SIZE as usize));
    for y in 0..MODEL_SIZE as usize {
        for x in 0..MODEL_SIZE as usize {
            let pixel = resized.get_pixel(x as u32, y as u32);
            input_tensor[[0, 0, y, x]] = pixel[0] as f32 / 255.0;
            input_tensor[[0, 1, y, x]] = pixel[1] as f32 / 255.0;
            input_tensor[[0, 2, y, x]] = pixel[2] as f32 / 255.0;
        }
    }

    if *stopped.lock().unwrap() {
        return Err("Cancelled".into());
    }

    let _ = app.emit("log", "Running background removal inference...");
    let input_tensor = input_tensor.into_dyn();
    let tensor = ort::value::Tensor::from_array(input_tensor)?;
    let outputs = session.run(ort::inputs!["input" => tensor])?;

    if *stopped.lock().unwrap() {
        return Err("Cancelled".into());
    }

    let _ = app.emit("log", "Postprocessing mask...");
    let output_key = outputs.keys().next().ok_or("No output from model")?;
    let (shape, data) = outputs[output_key].try_extract_tensor::<f32>()?;

    let mask_h = shape.get(2).copied().unwrap_or(MODEL_SIZE as i64) as u32;
    let mask_w = shape.get(3).copied().unwrap_or(MODEL_SIZE as i64) as u32;

    let mut mask_img = image::ImageBuffer::<image::Luma<u8>, _>::new(mask_w, mask_h);
    for (i, pixel) in mask_img.pixels_mut().enumerate() {
        if i < data.len() {
            let val = (data[i].clamp(0.0, 1.0) * 255.0) as u8;
            *pixel = image::Luma([val]);
        }
    }

    let mask_resized = image::imageops::resize(
        &mask_img,
        orig_w,
        orig_h,
        image::imageops::FilterType::Lanczos3,
    );

    if *stopped.lock().unwrap() {
        return Err("Cancelled".into());
    }

    let _ = app.emit("log", "Applying background mask...");
    let mut output = original;
    for y in 0..orig_h {
        for x in 0..orig_w {
            let alpha = mask_resized.get_pixel(x, y)[0];
            output.get_pixel_mut(x, y)[3] = alpha;
        }
    }

    let result = DynamicImage::ImageRgba8(output);
    let out_ext = Path::new(output_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let _ = app.emit("log", "Saving background-removed image...");
    match out_ext.as_str() {
        "jpg" | "jpeg" => {
            let mut bg = image::RgbaImage::new(orig_w, orig_h);
            for pixel in bg.pixels_mut() {
                *pixel = image::Rgba([255, 255, 255, 255]);
            }
            image::imageops::overlay(&mut bg, &result.to_rgba8(), 0, 0);
            DynamicImage::ImageRgba8(bg)
                .to_rgb8()
                .save_with_format(output_path, image::ImageFormat::Jpeg)?;
        }
        "webp" => {
            result.save_with_format(output_path, image::ImageFormat::WebP)?;
        }
        _ => {
            result.save(output_path)?;
        }
    }

    Ok(())
}
