#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use memetool_shared::{FileList, ImageData};
use std::fs;
use tauri::{Manager, Icon};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn list_directory(path: &str, limit: u32, offset: u32) -> Result<FileList, ()> {
    let allowed_extensions: Vec<&str> = vec!["png", "jpg", "gif", "jpeg"];

    let file_path = match path.trim() == "" {
        true => shellexpand::tilde("~/Downloads"),
        false => shellexpand::tilde(path),
    };

    let paths: Vec<std::fs::DirEntry> = match fs::read_dir(file_path.to_string()) {
        Ok(val) => val
            .filter_map(|f| match f {
                Ok(val) => Some(val),
                Err(_) => None,
            })
            .collect(),
        Err(err) => {
            eprintln!("Failed to load dir {}: {:?}", path, err);
            return Ok(FileList::empty());
        }
    };
    let mut files: Vec<String> = vec![];

    for path in paths {
        let lower_path = path.path().display().to_string().to_ascii_lowercase();

        if !allowed_extensions
            .iter()
            .any(|e| lower_path.ends_with(format!(".{}", e).as_str()))
        {
            continue;
        }
        files.push(path.path().to_str().unwrap().to_string());
    }
    let total_files = files.len();
    let files = &files.as_slice()[(offset as usize)..((offset + limit) as usize)];

    Ok(FileList {
        files: files.to_vec(),
        total_files,
    })
}

#[tauri::command]
async fn get_image(path: &str) -> Result<ImageData, ()> {
    // data:image/png;base64
    ImageData::try_from_filepath(path).await.map_err(|_| ())
}

#[tokio::main]
async fn main() {
    tauri::async_runtime::set(tokio::runtime::Handle::current());


    // let icon_path = std::path::PathBuf::from("icons/apple-touch-icon-base.png");

    // let icon = Icon::File(icon_path);

    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            app.get_window("main").unwrap().open_devtools();
            // if let Err(err) = app.get_window("main").unwrap().set_icon(icon) {
            //     eprintln!("Failed to set icon: {err:?}");
            // };

            if let Err(err) = app.get_window("main").unwrap().maximize() {
                eprintln!("Failed to maximize window: {err:?}");
            };
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_directory, get_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
