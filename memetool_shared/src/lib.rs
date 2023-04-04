use std::fs::File;
use std::io::BufReader;

use image::io::Reader as ImageReader;
use serde::{Deserialize, Serialize};

pub const RESIZE_DEFAULTS: (u32, u32) = (800, 800);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct FileList {
    pub files: Vec<String>,
    pub total_files: usize,
}

impl FileList {
    pub fn empty() -> Self {
        Self {
            files: vec![],
            total_files: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    WebP,
    Pnm,
    Tiff,
    Tga,
    Dds,
    Bmp,
    Ico,
    Hdr,
    OpenExr,
    Farbfeld,
    Avif,
    Unknown,
}

impl From<ImageFormat> for image::ImageFormat {
    fn from(input: ImageFormat) -> image::ImageFormat {
        match input {
            ImageFormat::Png => image::ImageFormat::Png,
            ImageFormat::Jpeg => image::ImageFormat::Jpeg,
            ImageFormat::Gif => image::ImageFormat::Gif,
            ImageFormat::WebP => image::ImageFormat::WebP,
            ImageFormat::Pnm => image::ImageFormat::Pnm,
            ImageFormat::Tiff => image::ImageFormat::Tiff,
            ImageFormat::Tga => image::ImageFormat::Tga,
            ImageFormat::Dds => image::ImageFormat::Dds,
            ImageFormat::Bmp => image::ImageFormat::Bmp,
            ImageFormat::Ico => image::ImageFormat::Ico,
            ImageFormat::Hdr => image::ImageFormat::Hdr,
            ImageFormat::OpenExr => image::ImageFormat::OpenExr,
            ImageFormat::Farbfeld => image::ImageFormat::Farbfeld,
            ImageFormat::Avif => image::ImageFormat::Avif,
            ImageFormat::Unknown => panic!("This shouldn't be done!"),
        }
    }
}
impl From<image::ImageFormat> for ImageFormat {
    fn from(input: image::ImageFormat) -> Self {
        match input {
            image::ImageFormat::Png => ImageFormat::Png,
            image::ImageFormat::Jpeg => ImageFormat::Jpeg,
            image::ImageFormat::Gif => ImageFormat::Gif,
            image::ImageFormat::WebP => ImageFormat::WebP,
            image::ImageFormat::Pnm => ImageFormat::Pnm,
            image::ImageFormat::Tiff => ImageFormat::Tiff,
            image::ImageFormat::Tga => ImageFormat::Tga,
            image::ImageFormat::Dds => ImageFormat::Dds,
            image::ImageFormat::Bmp => ImageFormat::Bmp,
            image::ImageFormat::Ico => ImageFormat::Ico,
            image::ImageFormat::Hdr => ImageFormat::Hdr,
            image::ImageFormat::OpenExr => ImageFormat::OpenExr,
            image::ImageFormat::Farbfeld => ImageFormat::Farbfeld,
            image::ImageFormat::Avif => ImageFormat::Avif,
            _ => ImageFormat::Unknown,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct ImageData {
    pub content_type: String,
    pub file_path: String,
    pub file_url: Option<String>,
    pub file_size: Option<u64>,
    pub file_dimensions: Option<(u32, u32)>,
    pub file_type: Option<ImageFormat>,
}

impl ImageData {
    fn load_image(path: &str) -> Result<ImageReader<BufReader<File>>, String> {
        match ImageReader::open(path) {
            Ok(val) => match val.with_guessed_format() {
                Ok(val) => Ok(val),
                Err(err) => Err(format!(
                    "Failed to identify format of image {path}: {err:?}"
                )),
            },
            Err(err) => Err(format!("Failed to read image from {path}: {err:?}")),
        }
    }

    pub async fn try_from_imagepassed(image_data: ImagePassed) -> Result<Self, String> {
        // println!("try_from_imagepassed: {image_data:?}");
        let path = image_data.path;
        let content_type = mime_guess::from_path(&path);

        let file_path = std::path::PathBuf::from(path.clone());
        if !file_path.exists() {
            return Err(format!("Failed to find file {file_path:?}"));
        }
        if !file_path.is_file() {
            return Err(format!("Path is not file: {file_path:?}"));
        }

        let file_metadata = file_path
            .metadata()
            .map_err(|e| format!("Failed to get file metadata for {file_path:?}: {e:?}"))
            .unwrap();

        if !file_metadata.is_file() {
            return Err(format!("File path {path} is not file!"));
        }

        let file_dimensions = match ImageData::load_image(&path)?.into_dimensions() {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Failed to read dimensions of {path}: {err:?}");
                (0, 0)
            }
        };

        let res = Self {
            file_path: path,
            content_type: content_type.first().unwrap().to_string(),
            file_size: Some(file_metadata.len()),
            file_dimensions: Some(file_dimensions),
            file_url: Some(image_data.file_url.to_string()),
            file_type: image_data.image_format,
        };
        // eprintln!("image load result {res:?}");
        Ok(res)
    }
}

#[derive(Serialize, Deserialize)]
pub struct PathArgs<'a> {
    pub path: &'a str,
    pub limit: u32,
    pub offset: u32,
}

impl From<&ImageData> for ImagePassed {
    fn from(input: &ImageData) -> ImagePassed {
        ImagePassed {
            path: input.file_path.to_owned(),
            file_url: input.file_url.to_owned().unwrap(),
            image_format: None,
        }
    }
}

impl From<ImagePassed> for ImageData {
    fn from(input: ImagePassed) -> ImageData {
        ImageData {
            content_type: "".to_string(),
            file_path: input.path,
            file_size: None,
            file_dimensions: None,
            file_type: None,
            file_url: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ImagePassed {
    pub path: String,
    pub file_url: String,
    pub image_format: Option<ImageFormat>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum ImageAction {
    Delete,
    Resize { x: u32, y: u32 },
    Rename { new_path: String },
}
