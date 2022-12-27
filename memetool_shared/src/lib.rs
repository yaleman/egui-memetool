use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ImageData {
    pub content_type: String,
    pub file_path: String,
    pub file_url: Option<String>,
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

        Ok(Self {
            file_path: path.to_string(),
            content_type: content_type.first().unwrap().to_string(),
            file_url: None,
        })
    }
}



#[derive(Serialize, Deserialize)]
pub struct PathArgs<'a> {
    pub path: &'a str,
    pub limit: u32,
    pub offset: u32,
}

