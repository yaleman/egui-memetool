use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct FileList {
    pub files: Vec<String>,
    pub total_files: usize,
}

// impl From<Vec<String>> for FileList {
//     fn from(input: Vec<String>) -> Self {
//         FileList {
//             total_files: input.len(),
//             files: input,
//         }
//     }
// }

impl FileList {
    pub fn empty() -> Self {
        Self {
            files: vec![],
            total_files: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageData {
    pub content_type: String,
    pub data: String,
}

impl ImageData {
    pub async fn try_from_filepath(path: &str) -> Result<Self, String> {
        let content_type = mime_guess::from_path(path);

        let file_path = std::path::PathBuf::from(path);
        if !file_path.exists() {
            return Err(format!("Failed to find file {file_path:?}"));
        }
        if !file_path.is_file() {
            return Err(format!("Path is not file: {file_path:?}"));
        }

        let contents = std::fs::read(file_path)
            .map_err(|e| format!("Failed to read file: {e:?}"))
            .unwrap();

        let data = base64::encode(&contents);

        Ok(Self {
            data,
            content_type: content_type.first().unwrap().to_string(),
        })
    }
    /// This is what should go into the src attribute for the `img` tag.
    pub fn as_data(&self) -> String {
        format!("data:{};base64,{}", self.content_type, self.data)
    }
}
