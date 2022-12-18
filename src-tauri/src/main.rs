#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::fs;

use serde::{Serialize, Deserialize};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
// #[tauri::command]
// fn greet(name: &str) -> String {
//     format!("Hello, {}! You've been greeted from Rust!", name)
// }

#[derive(Deserialize, Serialize)]
pub struct FileList {
    files: Vec<String>
}

#[tauri::command]
fn list_directory(path: &str) -> FileList {
    let paths = fs::read_dir(path).unwrap();
    let mut files: Vec<String> = vec![];
    for path in paths {
        files.push(format!("{}", path.unwrap().path().display()));
    }
    FileList{ files }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_directory])
        // .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
