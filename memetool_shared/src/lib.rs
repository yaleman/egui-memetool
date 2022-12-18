use serde::{Deserialize,Serialize};

#[derive(Deserialize, Serialize)]
pub struct FileList {
    pub files: Vec<String>
}

impl From<Vec<String>> for FileList {
    fn from(input: Vec<String>) -> Self {
        FileList { files: input }
    }
}

impl FileList {
    pub fn empty() -> Self {
        Self { files: vec![] }
    }
}