#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use eframe::epaint::Vec2;
use memetool::{THUMBNAIL_SIZE, GRID_X, GRID_Y};
use memetool::background::background;
use tokio::runtime::Runtime;


fn main() -> Result<(), eframe::Error> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "INFO");
    }

    let rt = Runtime::new().expect("Unable to create Runtime");
    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    let (foreground_tx, foreground_rx) = tokio::sync::mpsc::channel(100);
    let (background_tx, background_rx) = tokio::sync::mpsc::channel(100);

    // Execute the runtime in its own thread.
    rt.spawn(background(background_rx, foreground_tx));

    // let app_icon = include_bytes!("../assets/app-icon.png");
    // let app_icon = match image::load_from_memory(app_icon) {
    //     Ok(val) => val,
    //     Err(err) => {
    //         error!("Failed to load app icon: {:?}", err);
    //         panic!();
    //     }
    // };

    // let app_icon = IconData {
    //     rgba: app_icon.to_rgb8().to_vec(),
    //     width: 512,
    //     height: 512,
    // };

    // calculating the window size for great profit
    let min_window_size = Some(Vec2::new(
        THUMBNAIL_SIZE.x * *GRID_X as f32,
        THUMBNAIL_SIZE.y * (*GRID_Y as f32 + 1.2),
    ));

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        decorated: true,
        // drag_and_drop_support: todo!(),
        icon_data: None, // Some(app_icon),
        // initial_window_pos: todo!(),
        min_window_size,
        // max_window_size: todo!(),
        resizable: true,
        transparent: false,
        // mouse_passthrough: todo!(),
        // vsync: todo!(),
        // multisampling: todo!(),
        // depth_buffer: todo!(),
        // stencil_buffer: todo!(),
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::Glow,
        follow_system_theme: true,
        default_theme: eframe::Theme::Light,
        // run_and_return: todo!(),
        // event_loop_builder: todo!(),
        // shader_version: todo!(),
        centered: false,
        ..Default::default()
    };

    eframe::run_native(
        "memetool",
        options,
        Box::new(|cc| Box::new(memetool::MemeTool::new(cc, foreground_rx, background_tx))),
    )
}
