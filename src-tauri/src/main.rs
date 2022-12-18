#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::fs;
use memetool_shared::FileList;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
// #[tauri::command]
// fn greet(name: &str) -> String {
//     format!("Hello, {}! You've been greeted from Rust!", name)
// }

#[tauri::command]
fn list_directory(path: &str) -> FileList {

    let allowed_extensions: Vec<&str> = vec![
        "png",
        "jpg",
        "gif",
        "jpeg",
    ];

    let file_path = match path.trim() == "" {
        true => shellexpand::tilde("~/Downloads"),
        false => shellexpand::tilde(path)
    };

    let paths = match fs::read_dir(file_path.to_string()){
        Ok(val) => val,
        Err(err) => {
            eprintln!("Failed to load dir {}: {:?}", path, err);
            return FileList::empty();
        }
    };
    let mut files: Vec<String> = vec![];
    for path in paths {
        if let Ok(val) = path {
            let lower_path = val.path().display().to_string().to_ascii_lowercase();
            let mut ok_filetype = false;
            for ext in allowed_extensions.iter() {
                if lower_path.ends_with(format!(".{}", ext).as_str())  {
                    ok_filetype = true;
                    continue
                }
            }
            if !ok_filetype {
                continue
            }
            files.push(format!("{}", val.path().display()));
        }
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
