#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, RichText, TextStyle};
use eframe::epaint::{FontFamily, FontId, Vec2};
use egui_extras::RetainedImage;
use log::*;
use itertools::Itertools;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, Sender};


mod image;
use crate::image::load_image_to_thumbnail;


lazy_static! {
    static ref OK_EXTENSIONS: Vec<&'static str> = vec!["jpg", "gif", "png", "jpeg",];
    static ref PER_PAGE: usize = 20;
    static ref GRID_X: u8 = 5;
    static ref GRID_Y: u8 = 4;
    static ref THUMBNAIL_SIZE: Vec2 = Vec2 { x: 200.0, y: 150.0 };
}

#[inline]
fn heading2() -> TextStyle {
    TextStyle::Name("Heading2".into())
}

#[inline]
fn heading3() -> TextStyle {
    TextStyle::Name("ContextHeading".into())
}

fn configure_text_styles(ctx: &egui::Context) {
    use FontFamily::{Monospace, Proportional};

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(25.0, Proportional)),
        (heading2(), FontId::new(22.0, Proportional)),
        (heading3(), FontId::new(19.0, Proportional)),
        (TextStyle::Body, FontId::new(16.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
        (TextStyle::Button, FontId::new(12.0, Proportional)),
        (TextStyle::Small, FontId::new(8.0, Proportional)),
    ]
    .into();
    ctx.set_style(style);
}

#[derive(Clone)]
enum AppState {
    Browser,
    Editor {
        filepath: String,
        image: Arc<RetainedImage>,
    },
}

// #[derive(Eq, PartialEq)]
enum AppMsg {
    ThumbImageResponse(ThumbImageResponse),
    NewAppState(AppState),
}

struct ThumbImageResponse {
    filepath: String,
    page: usize,
    image: Arc<RetainedImage>,
}

struct MemeTool {
    /// Current working directory
    pub workdir: String,
    pub files_list: Vec<PathBuf>,
    pub current_page: usize,
    pub app_state: AppState,
    pub last_checked: Option<String>,
    pub per_page: usize,
    pub browser_images: HashMap<String, ThumbImageResponse>, // TODO: this should be a hashmap of filename, image
    pub background_rx: Receiver<AppMsg>,
    pub background_tx: Sender<AppMsg>,
}

impl eframe::App for MemeTool {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(msg) = self.background_rx.try_recv() {
            match msg {
                AppMsg::ThumbImageResponse(image_response) => {
                    if image_response.page == self.current_page {
                        debug!("got response for: {}", image_response.filepath);
                        self.browser_images
                            .insert(image_response.filepath.clone(), image_response);
                        ctx.request_repaint_after(Duration::from_millis(100));
                    } else {
                        debug!(
                            "Got a message for page {}, but we're on page {}",
                            image_response.page, self.current_page
                        );
                    }
                }
                AppMsg::NewAppState(new_state) => {
                    self.app_state = new_state;
                    ctx.request_repaint();
                }
            }
        }

        match &self.app_state {
            AppState::Browser => self.show_browser(ctx.clone()),
            AppState::Editor { filepath, image } => {
                // eprintln!("Showing editor: {}", filepath);
                egui::CentralPanel::default().show(ctx, |ui| {
                    if ui.button("Back").clicked() {
                        let tx = self.background_tx.clone();
                        tokio::spawn(async move {
                            if tx
                                .send(AppMsg::NewAppState(AppState::Browser))
                                .await
                                .is_err()
                            {
                                eprintln!("Tried to update appstate and failed!");
                            };
                        });
                    };
                    ui.label(filepath);

                    image.show(ui);
                });
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl MemeTool {
    /// sets some things up
    fn new(
        cc: &eframe::CreationContext<'_>,
        // background_rx: Receiver<BackgroundMessage>,
        // background_tx: Sender<BackgroundMessage>,
    ) -> Self {


        pretty_env_logger::init();


        configure_text_styles(&cc.egui_ctx);

        let (background_tx, background_rx) = tokio::sync::mpsc::channel(100);

        Self {
            background_rx,
            background_tx,
            workdir: "~/Downloads".into(),
            files_list: vec![],
            current_page: 0,
            app_state: AppState::Browser,
            last_checked: None,
            per_page: *PER_PAGE,
            browser_images: HashMap::new(),
        }
    }

    /// Get a given page of file results
    fn get_page(&self) -> Vec<PathBuf> {
        if self.files_list.len() <= self.per_page {
            self.files_list.clone()
        } else {
            match self.files_list.chunks(self.per_page).nth(self.current_page) {
                Some(list) => list.to_vec(),
                None => vec![],
            }
        }
    }

    // build a threaded promisey thing to update images in the backend.
    fn start_update(&mut self, ctx: &egui::Context) {
        // TODO: maybe set an upper bound on the cache?

        let resolvedpath = shellexpand::tilde(&self.workdir);
        self.files_list = match std::fs::read_dir(resolvedpath.to_string()) {
            Ok(dirlist) => dirlist
                .sorted_by_key(|d| {
                    d.as_ref()
                        .unwrap()
                        .file_name()
                        .into_string()
                        .unwrap_or("".into())
                })
                .filter_map(|filename| match filename {
                    Ok(val) => {
                        let pathstr = val.path();
                        let pathstr = pathstr.to_string_lossy().to_lowercase();
                        if OK_EXTENSIONS
                            .iter()
                            .any(|ext| pathstr.ends_with(&format!(".{ext}")))
                        {
                            Some(val.path())
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                })
                .collect(),
            Err(_) => vec![],
        };
        eprintln!("Starting update in thread...");

        self.get_page().iter().for_each(|filepath| {
            send_req(
                self.current_page,
                filepath.to_owned(),
                self.background_tx.clone(),
                ctx.clone(),
            );
            // eprintln!("Sent message for: {}", filepath.display());
        });
    }

    fn show_browser(&mut self, ctx: egui::Context) {
        // println!("starting show_browser repaint");
        egui::CentralPanel::default().show(&ctx, |ui| {
            match &self.last_checked {
                Some(val) => {
                    if val != &self.workdir {
                        println!("{} != {}", val, self.workdir);
                        self.start_update(&ctx)
                    }
                }
                None => {
                    self.start_update(&ctx);
                }
            };
            self.last_checked = Some(self.workdir.clone());

            ui.horizontal(|ui| {
                let name_label = ui.label(
                    RichText::new("Current workdir: ")
                        .text_style(heading3())
                        .strong(),
                );
                ui.text_edit_singleline(&mut self.workdir)
                    .labelled_by(name_label.id);
            });

            ui.add_space(15.0);
            ui.horizontal(|ui| {
                if ui.button("Prev Page").clicked() {
                    eprintln!("Pref page clicked");
                    if self.current_page > 0 {
                        self.current_page -= 1;
                        self.last_checked = None;
                    }
                }
                ui.add_space(15.0);
                if ui.button("Next Page").clicked() {
                    eprintln!("Next page clicked");
                    if self.current_page < (self.files_list.len() / self.per_page) {
                        self.current_page += 1;
                        self.last_checked = None;
                    }
                }
            });
            ui.add_space(15.0);

            let mut loaded_images = 0;

            egui::Grid::new("browser")
                .num_columns(5)
                .spacing([10.0, 10.0]) // grid spacing
                .show(ui, |ui| {
                    let mut col = 0;
                    let filenames: Vec<String> = self
                        .get_page()
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();

                    filenames.into_iter().sorted().for_each(|filename| {
                        if let Some(i) = self.browser_images.get(&filename) {
                            let image = i.image.show_max_size(ui, *THUMBNAIL_SIZE);
                            let imageresponse = image.interact(egui::Sense::click());
                            if imageresponse.clicked() {
                                self.app_state = AppState::Editor {
                                    filepath: i.filepath.clone(),
                                    image: i.image.clone(),
                                };
                            };
                            col += 1;
                            if col > 4 {
                                col = 0;
                                ui.end_row();
                            }
                            loaded_images += 1;
                        }
                    });
                });

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.label(format!("Number of files: {}", self.files_list.len()));
                let last_checked = &self.last_checked.as_ref();
                ui.label(format!(
                    "Last Checked: {}",
                    &last_checked.unwrap_or(&"".to_string())
                ));
                ui.label(format!("Current page: {}", self.current_page + 1));
                if loaded_images != self.get_page().len() {
                    ui.label(format!(
                        "Loading images... {}/{}",
                        loaded_images,
                        self.get_page().len()
                    ));
                };


            });
        });
    }
}

fn send_req(page: usize, filepath: PathBuf, tx: Sender<AppMsg>, ctx: egui::Context) {
    puffin::profile_scope!("image loader");
    tokio::spawn(async move {
        // Send a request with an increment value.
        let response = AppMsg::ThumbImageResponse(ThumbImageResponse {
            filepath: filepath.display().to_string(),
            page,
            image: Arc::new(load_image_to_thumbnail(&filepath).unwrap()),
        });
        // eprintln!("Sending response for {}", filepath.display());
        let _ = tx.send(response).await;
        // After parsing the response, notify the GUI thread
        ctx.request_repaint_after(Duration::from_millis(100));
    });
}

// fn update_files_list(workdir: String, current_page: usize) -> Box<MemeTool> {
//     eprintln!("***** UPDATING FILES LIST from {} *****", workdir);
//     let resolvedpath = shellexpand::tilde(&workdir);

//     let files_list = match std::fs::read_dir(resolvedpath.to_string()) {
//         Ok(dirlist) => dirlist
//             .filter_map(|filename| match filename {
//                 Ok(val) => {
//                     let pathstr = val.path().clone();
//                     let pathstr = pathstr.to_string_lossy().to_lowercase();
//                     if OK_EXTENSIONS
//                         .iter()
//                         .any(|ext| pathstr.ends_with(&format!(".{ext}")))
//                     {
//                         Some(val.path().to_path_buf().to_owned())
//                     } else {
//                         None
//                     }
//                 }
//                 Err(_) => None,
//             })
//             .collect(),
//         Err(_) => vec![],
//     };
//     let mut memetool = MemeTool {
//         current_page,
//         workdir: workdir.clone(),
//         last_checked: Some(workdir),
//         files_list,
//         ..Default::default()
//     };

//     memetool.browser_images = memetool
//         .get_page()
//         .par_iter()
//         .filter_map(|filepath| match load_image_to_thumbnail(&filepath) {
//             Ok(val) => Some(Arc::new(val)),
//             Err(err) => {
//                 eprintln!("Failed to load {}: {}", filepath.to_string_lossy(), err);
//                 None
//             }
//         })
//         .collect();
//     Box::new(memetool)
// }

fn main() -> Result<(), eframe::Error> {
    let rt = Runtime::new().expect("Unable to create Runtime");
    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    // Execute the runtime in its own thread.
    // The future doesn't have to do anything. In this example, it just sleeps forever.
    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        })
    });

    // calculating the window size for great profit
    let min_window_size = Some(Vec2::new(
        THUMBNAIL_SIZE.x * *GRID_X as f32,
        THUMBNAIL_SIZE.y * (*GRID_Y as f32 + 1.2),
    ));

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        decorated: true,
        // drag_and_drop_support: todo!(),
        // icon_data: Some(IconData::from("hello world")),
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
        Box::new(|cc| Box::new(MemeTool::new(cc))),
    )
}
