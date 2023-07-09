use std::path::PathBuf;

use eframe::egui;
use eframe::epaint::{ColorImage, Vec2};
use egui_extras::RetainedImage;
use image::Pixel;
use log::*;

use crate::THUMBNAIL_SIZE;

pub async fn load_image_to_thumbnail_async(
    filename: &PathBuf,
    size: Option<Vec2>,
) -> Result<RetainedImage, String> {
    debug!("Loading {}", filename.to_string_lossy());

    use tokio::fs::File;
    use tokio::io::AsyncReadExt; // for read_to_end()
    let mut file = match File::open(filename).await {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open file: {}", e);
            return Err(e.to_string());
        }
    };

    let mut contents = vec![];
    if let Err(err) = file.read_to_end(&mut contents).await {
        error!("Failed to read file: {}", err);
        return Err(err.to_string());
    }

    let image = image::load_from_memory(&contents)
        .map_err(|e| e.to_string())?;

    let (x, y) = match size {
        Some(size) => (size.x as u32, size.y as u32),
        None => (THUMBNAIL_SIZE.x as u32, THUMBNAIL_SIZE.y as u32),
    };

    let image = image.thumbnail(x, y);

    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();

    let ci = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

    let response = egui_extras::RetainedImage::from_color_image(filename.to_string_lossy(), ci);
    debug!("Finished loading {}", filename.display());
    Ok(response)
}

pub fn load_image_to_thumbnail(
    filename: &PathBuf,
    size: Option<Vec2>,
) -> Result<RetainedImage, String> {
    debug!("Loading {}", filename.to_string_lossy());
    puffin::profile_function!(filename.display().to_string());
    let image = image::io::Reader::open(filename)
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;

    let (x, y) = match size {
        Some(size) => (size.x as u32, size.y as u32),
        None => (THUMBNAIL_SIZE.x as u32, THUMBNAIL_SIZE.y as u32),
    };

    let image = image.thumbnail(x, y);

    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();

    let ci = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

    let response = egui_extras::RetainedImage::from_color_image(filename.to_string_lossy(), ci);
    debug!("Finished loading {}", filename.display());
    Ok(response)
}

/// throw some pixels at it, get a texture back
pub fn load_image_from_memory(image_data: &[u8]) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

pub fn optimize_image(filename: impl ToString) {
    let image_object = image::open(filename.to_string())
        .unwrap();
    // let image_buffer = image_object
    //     .to_rgba8().save_with_format(filename.to_string(), image::ImageFormat::Png).unwrap();

}