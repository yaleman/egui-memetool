use std::path::PathBuf;

use eframe::epaint::ColorImage;
use egui_extras::RetainedImage;
use log::*;

use crate::THUMBNAIL_SIZE;

pub fn load_image_to_thumbnail(filename: &PathBuf) -> Result<RetainedImage, String> {
    debug!("Loading {}", filename.to_string_lossy());
    puffin::profile_function!(filename.display().to_string());
    let image = image::io::Reader::open(filename)
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;

    let image = image.thumbnail(THUMBNAIL_SIZE.x as u32, THUMBNAIL_SIZE.y as u32);

    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();

    let ci = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

    let response = egui_extras::RetainedImage::from_color_image(filename.to_string_lossy(), ci);
    debug!("Finished loading {}", filename.display());
    Ok(response)
}
