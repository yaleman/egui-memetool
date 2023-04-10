#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, RichText, TextStyle, TextureOptions};
use eframe::epaint::{vec2, FontFamily, FontId, Vec2};
use eframe::IconData;
use egui_extras::RetainedImage;
use itertools::Itertools;
use log::*;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, Sender};

mod image_utils;
use crate::image_utils::load_image_to_thumbnail;

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
    Editor { filepath: String },
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
    loading_image: egui::TextureHandle,
}

impl eframe::App for MemeTool {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(msg) = self.background_rx.try_recv() {
            match msg {
                AppMsg::ThumbImageResponse(image_response) => {
                    debug!(
                        "got response for: filepath={} page={}",
                        image_response.filepath, image_response.page
                    );
                    self.browser_images
                        .insert(image_response.filepath.clone(), image_response);
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                AppMsg::NewAppState(new_state) => {
                    self.app_state = new_state;
                    ctx.request_repaint();
                }
            }
        }

        match &self.app_state {
            AppState::Browser => self.show_browser(ctx.clone()),
            AppState::Editor { filepath } => self.show_editor(ctx.clone(), filepath),
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn load_image_from_memory(image_data: &[u8]) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

impl MemeTool {
    /// sets some things up
    fn new(
        cc: &eframe::CreationContext<'_>,
        // background_rx: Receiver<BackgroundMessage>,
        // background_tx: Sender<BackgroundMessage>,
    ) -> Self {
        pretty_env_logger::init();

        let loading_image = cc.egui_ctx.load_texture(
            "loading_image",
            load_image_from_memory(include_bytes!("../assets/app-icon.png")).unwrap(),
            TextureOptions::default(),
        );

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
            loading_image,
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

    fn update_files_list(&mut self) {
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
    }

    // build a threaded promisey thing to update images in the backend.
    fn start_update(&mut self, ctx: &egui::Context) {
        // TODO: maybe set an upper bound on the cache?
        self.update_files_list();

        debug!("Starting update in thread...");

        self.get_page().iter().for_each(|filepath| {
            send_req(
                self.current_page,
                filepath.to_owned(),
                self.background_tx.clone(),
                ctx.clone(),
            );
            debug!("Sent message for: {}", filepath.display());
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
                if self.current_page > 0 {
                    if ui.button("First Page").clicked() {
                        self.current_page = 0;
                        self.last_checked = None;
                    };

                    if ui.button("Prev Page").clicked() {
                        debug!("Pref page clicked");
                        self.current_page -= 1;
                        self.last_checked = None;
                    }
                    ui.add_space(15.0);
                }

                if ui.button("Next Page").clicked() {
                    debug!("Next page clicked");
                    if self.current_page < (self.files_list.len() / self.per_page) {
                        self.current_page += 1;
                        self.last_checked = None;
                    }
                }
            });
            ui.add_space(15.0);

            let mut loaded_images = 0;

            egui::Grid::new("browser")
                .num_columns(10)
                .spacing([10.0, 10.0]) // grid spacing
                .show(ui, |ui| {
                    let mut col = 0;
                    let filenames: Vec<String> = self
                        .get_page()
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();

                    filenames.into_iter().sorted().for_each(|filename| {
                        let image = match self.browser_images.get(&filename) {
                            Some(i) => {
                                let space =
                                    ((THUMBNAIL_SIZE.x - i.image.width() as f32) / 2.0) + 1.0;
                                ui.add_space(space);
                                i.image.show_max_size(ui, *THUMBNAIL_SIZE)
                            }
                            None => {
                                ui.add_space((THUMBNAIL_SIZE.x - THUMBNAIL_SIZE.y) / 2.0);
                                ui.image(
                                    self.loading_image.id(),
                                    vec2(THUMBNAIL_SIZE.y, THUMBNAIL_SIZE.y),
                                )
                            }
                        };
                        let imageresponse = image.interact(egui::Sense::click());
                        if imageresponse.clicked() {
                            self.app_state = AppState::Editor { filepath: filename };
                        };

                        col += 1;
                        if col > 4 {
                            col = 0;
                            ui.end_row();
                        }
                        loaded_images += 1;
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

    fn show_editor(&self, ctx: egui::Context, filepath: &String) {
        info!("Showing editor: {}", filepath);
        let image = match self.browser_images.get(filepath) {
            Some(i) => i.image.clone(),
            None => todo!(),
        };
        egui::CentralPanel::default().show(&ctx, |ui| {
            if ui.button("Back").clicked() {
                let tx = self.background_tx.clone();
                tokio::spawn(async move {
                    if tx
                        .send(AppMsg::NewAppState(AppState::Browser))
                        .await
                        .is_err()
                    {
                        error!("Tried to update appstate and failed!");
                    };
                });
            };
            ui.label(filepath);

            image.show(ui);
        });
    }
}

fn send_req(page: usize, filepath: PathBuf, tx: Sender<AppMsg>, ctx: egui::Context) {
    puffin::profile_scope!("image loader");
    tokio::spawn(async move {
        // Send a request with an increment value.
        let image = match load_image_to_thumbnail(&filepath) {
            Ok(image) => image,
            Err(err) => {
                error!("Failed to load {} {}", filepath.display(), err);
                return;
            }
        };

        let response = AppMsg::ThumbImageResponse(ThumbImageResponse {
            filepath: filepath.display().to_string(),
            page,
            image: Arc::new(image),
        });
        trace!("Sending response for {}", filepath.display());
        let _ = tx.send(response).await;
        // After parsing the response, notify the GUI thread
        ctx.request_repaint_after(Duration::from_millis(100));
    });
}

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

    let app_icon = include_bytes!("../assets/app-icon.png");
    let app_icon = match image::load_from_memory(app_icon) {
        Ok(val) => val,
        Err(err) => {
            error!("Failed to load app icon: {:?}", err);
            panic!();
        }
    };

    let app_icon = IconData {
        rgba: app_icon.to_rgb8().to_vec(),
        width: 512,
        height: 512,
    };

    // calculating the window size for great profit
    let min_window_size = Some(Vec2::new(
        THUMBNAIL_SIZE.x * *GRID_X as f32,
        THUMBNAIL_SIZE.y * (*GRID_Y as f32 + 1.2),
    ));

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        decorated: true,
        // drag_and_drop_support: todo!(),
        icon_data: Some(app_icon),
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
