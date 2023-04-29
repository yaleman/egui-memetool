#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, Context, Key, RichText, TextureOptions};
use eframe::epaint::{vec2, Vec2};
use eframe::IconData;
use egui_extras::RetainedImage;
use itertools::Itertools;
use log::*;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, Sender};

mod background;
mod image_utils;
mod s3_upload;
mod text;
use crate::background::*;
use crate::image_utils::{load_image_from_memory, load_image_to_thumbnail};
use text::{configure_text_styles, heading3};

lazy_static! {
    static ref OK_EXTENSIONS: Vec<&'static str> = vec!["jpg", "gif", "png", "jpeg",];
    static ref PER_PAGE: usize = 20;
    static ref GRID_X: u8 = 5;
    static ref GRID_Y: u8 = 4;
    static ref GRID_SPACING: Vec2 = Vec2 { x: 10.0, y: 10.0 };
    static ref THUMBNAIL_SIZE: Vec2 = Vec2 { x: 200.0, y: 150.0 };
}

#[derive(Clone, Debug)]
pub enum AppState {
    Browser,
    Editor {
        filepath: String,
    },
    RenameConfirm {
        filepath: String,
        newfilepath: String,
    },
    ShowError {
        message: String,
        next_state: Option<Box<AppState>>,
    },
    DeletePrompt(String),
    UploadPrompt(String),
    Uploading(String),
}

#[derive(Debug)]
pub enum AppMsg {
    LoadImage(ThumbImageMsg),
    ThumbImageResponse(ThumbImageMsg),
    ImageLoadFailed { filename: String, error: String },
    NewAppState(AppState),
    Echo(String),
    UploadImage(String),
    UploadComplete(String),
    Error(String),
}

pub struct ThumbImageMsg {
    filepath: String,
    page: usize,
    image: Option<Arc<RetainedImage>>,
}

impl core::fmt::Debug for ThumbImageMsg {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThumbImageResponse")
            .field("filepath", &self.filepath)
            .field("page", &self.page)
            .finish()
    }
}

struct MemeTool {
    /// Current working directory
    pub workdir: String,
    pub files_list: Vec<PathBuf>,
    pub current_page: usize,
    pub app_state: AppState,
    last_checked_dir: Option<String>,
    last_checked_page: Option<usize>,
    pub per_page: usize,
    pub browser_images: HashMap<String, ThumbImageMsg>,
    pub background_rx: Receiver<AppMsg>,
    pub background_tx: Sender<AppMsg>,
    loading_image: egui::TextureHandle,
    allow_shortcuts: bool,
    key_buffer: Vec<Key>,
    editor_image_cache: Option<RetainedImage>,
    editor_rename_target: String,
    editor_rename_has_focus: bool,
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
                    self.editor_rename_target = String::new();
                    self.editor_image_cache = None;

                    self.app_state = new_state;
                    ctx.request_repaint();
                }
                AppMsg::ImageLoadFailed { filename, error } => {
                    // TODO: some kind of herpaderp image error handler thingy?
                    error!("Failed to load image: {filename}: {error}");
                }
                AppMsg::Echo(msg) => debug!("Echo {}", msg),
                AppMsg::UploadImage(filepath) => {
                    error!("Backend sent UploadImage({})", filepath);
                }
                AppMsg::LoadImage(_) => {
                    error!("Backend sent LoadImage() which is bad.");
                }
                AppMsg::UploadComplete(filepath) => self.app_state = AppState::Editor { filepath },
                AppMsg::Error(err) => {
                    self.app_state = AppState::ShowError {
                        message: err,
                        next_state: None,
                    }
                }
            }
        }
        let app_state = self.app_state.clone();

        match app_state {
            AppState::Browser => self.show_browser(ctx.clone()),
            AppState::Editor { filepath } => self.show_editor(ctx.clone(), filepath.as_str()),
            AppState::RenameConfirm {
                filepath,
                newfilepath,
            } => self.show_rename_confirm(ctx.clone(), filepath, newfilepath),
            AppState::ShowError {
                message,
                next_state,
            } => self.show_error(ctx.clone(), message, next_state),
            AppState::DeletePrompt(filepath) => self.show_delete_prompt(ctx.clone(), filepath),
            AppState::UploadPrompt(filepath) => self.show_upload_prompt(ctx.clone(), filepath),
            AppState::Uploading(filepath) => self.show_uploading(ctx.clone(), filepath),
        };

        if self.allow_shortcuts && !ctx.wants_keyboard_input() {
            self.key_handler(ctx.clone());
        } else {
            trace!("Not allowing shorcuts!");
        }

        // ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl MemeTool {
    /// sets some things up
    fn new(
        cc: &eframe::CreationContext<'_>,
        background_rx: Receiver<AppMsg>,
        background_tx: Sender<AppMsg>,
    ) -> Self {
        pretty_env_logger::init();

        let loading_image = cc.egui_ctx.load_texture(
            "loading_image",
            load_image_from_memory(include_bytes!("../assets/app-icon.png")).unwrap(),
            TextureOptions::default(),
        );

        configure_text_styles(&cc.egui_ctx);

        Self {
            background_rx,
            background_tx,
            workdir: "~/Downloads".into(),
            files_list: vec![],
            current_page: 0,
            app_state: AppState::Browser,
            last_checked_dir: None,
            last_checked_page: None,
            per_page: *PER_PAGE,
            browser_images: HashMap::new(),
            loading_image,
            allow_shortcuts: true,
            key_buffer: vec![],
            editor_image_cache: None,
            editor_rename_target: String::new(),
            editor_rename_has_focus: false,
        }
    }

    // TODO: handle_next_page
    // TODO: handle_prev_page

    fn key_handler(&mut self, ctx: Context) {
        ctx.input(|input| {
            self.key_buffer.clone().iter().for_each(|key| {
                if input.key_released(key.to_owned()) {
                    debug!("released! {:?}", key);
                    match key {
                        // Key::ArrowDown => todo!(),
                        Key::Delete => {
                            // if we're in the editor, prompt for deletion
                            if let AppState::Editor { filepath } = &self.app_state {
                                self.app_state = AppState::DeletePrompt(filepath.clone());
                            }
                        }

                        Key::Enter => {}
                        Key::Escape => match &self.app_state {
                            AppState::Editor { .. } => {
                                debug!("User hit escape in editor...");
                                self.app_state = AppState::Browser;
                            }
                            AppState::RenameConfirm {
                                filepath,
                                newfilepath: _,
                            } => {
                                debug!("User hit escape in rename confirmation...");
                                self.app_state = AppState::Editor {
                                    filepath: filepath.clone(),
                                };
                            }
                            AppState::DeletePrompt(filepath) => {
                                debug!("User hit escape in delete prompt...");
                                self.app_state = AppState::Editor {
                                    filepath: filepath.clone(),
                                };
                            }
                            _ => {}
                        },
                        Key::ArrowLeft => {
                            if let AppState::Browser = self.app_state {
                                if self.current_page > 0 {
                                    self.current_page -= 1;
                                    // ctx.request_repaint_after(Duration::from_millis(100));
                                }
                            }
                        }
                        Key::ArrowRight => {
                            if let AppState::Browser = self.app_state {
                                // if self.current_page > 0 {
                                self.current_page += 1;
                                // ctx.request_repaint_after(Duration::from_millis(100));
                                // }
                            }
                        }
                        // Key::A => todo!(),
                        // Key::F1 => todo!(),
                        _ => {}
                    }
                }
            });
            self.key_buffer.clear();

            if !input.keys_down.is_empty() {
                for key in input.keys_down.iter() {
                    if !self.key_buffer.contains(key) {
                        self.key_buffer.push(key.to_owned());
                    }
                }
            }
        });
        // let events = ui.input().events.clone();
        //     for event in &events {
        //         // match event {
        //         //     egui::Event::Key{key, pressed, modifiers} => {
        //         //         println!("{:?} = {:?}", key, pressed);
        //         //     },
        //         //     egui::Event::Text(t) => { println!("Text = {:?}", t) }
        //         //     _ => {}
        //         // }
        //         debug!("Event: {event:?}");
        // }
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

        let cached_files: Vec<String> = self.browser_images.keys().map(|k| k.to_owned()).collect();

        // clear out the cached_Files that are no longer in the files_list
        for filename in cached_files {
            let filepath = PathBuf::from(&filename);
            if !self.files_list.contains(&filepath) {
                info!("Removing {} from cached files", filename);
                self.browser_images.remove(&filename);
            }
        }
    }

    /// build a threaded promisey thing to update images in the backend.
    fn start_update(&mut self, ctx: &egui::Context) {
        // TODO: maybe set an upper bound on the cache?
        self.update_files_list();

        debug!("Starting update in thread...");

        let current_page = self.current_page;

        self.get_page().into_iter().for_each(|filepath| {
            debug!("Sending message for: {}", filepath.display());
            let tx = self.background_tx.clone();
            tokio::spawn(async move {
                if let Err(err) = tx
                    .send(AppMsg::LoadImage(ThumbImageMsg {
                        filepath: filepath.display().to_string(),
                        page: current_page,
                        image: None,
                    }))
                    .await
                {
                    error!("Failed to send background message: {}", err.to_string());
                };
            });

            // Box::new(send_req(
            //     self.current_page,
            //     filepath.to_owned(),
            //     self.background_tx.clone(),
            //     ctx.clone(),
            // ))
        });
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn check_needs_update(&mut self, ctx: &egui::Context) {
        match (&self.last_checked_dir, &self.last_checked_page) {
            (Some(dir), Some(page)) => {
                if dir != &self.workdir || page != &self.current_page {
                    self.start_update(ctx)
                } else {
                    trace!("no update needed {} == {}", dir, self.workdir);
                }
            }
            (None, None) => {
                debug!("last_checked is None, starting update");
                self.start_update(ctx);
            }
            _ => {}
        };
        self.last_checked_dir = Some(self.workdir.clone());
        self.last_checked_page = Some(self.current_page);
    }

    fn show_browser(&mut self, ctx: egui::Context) {
        // println!("starting show_browser repaint");
        egui::CentralPanel::default().show(&ctx, |ui| {
            self.check_needs_update(&ctx);

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
                        self.last_checked_dir = None;
                    };

                    if ui.button("Prev Page").clicked() {
                        debug!("Pref page clicked");
                        self.current_page -= 1;
                        self.last_checked_dir = None;
                    }
                    ui.add_space(15.0);
                }

                if ui.button("Next Page").clicked() {
                    debug!("Next page clicked");
                    if self.current_page < (self.files_list.len() / self.per_page) {
                        self.current_page += 1;
                        self.last_checked_dir = None;
                    }
                }
            });
            ui.add_space(15.0);

            let mut loaded_images = 0;

            egui::Grid::new("browser")
                .num_columns(10)
                .spacing(*GRID_SPACING) // grid spacing
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
                                loaded_images += 1;
                                let img = i.image.clone().unwrap();
                                let space = ((THUMBNAIL_SIZE.x - img.width() as f32) / 2.0) + 1.0;
                                ui.add_space(space);
                                img.as_ref().show_max_size(ui, *THUMBNAIL_SIZE)
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
                            // reset the things
                            self.editor_image_cache = None;
                            self.editor_rename_target = String::new();
                            self.app_state = AppState::Editor { filepath: filename };
                        };

                        col += 1;
                        if col > 4 {
                            col = 0;
                            ui.end_row();
                        }
                    });
                });

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.label(format!("Number of files: {}", self.files_list.len()));
                let last_checked = &self.last_checked_dir.as_ref();
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
        ctx.request_repaint_after(Duration::from_micros(100));
    }

    fn show_error(
        &mut self,
        ctx: egui::Context,
        message: String,
        next_state: Option<Box<AppState>>,
    ) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.label(message);
            if ui.button("Continue").clicked() {
                if let Some(state) = next_state {
                    self.app_state = *state;
                } else {
                    self.app_state = AppState::Browser;
                }
            };
        });
    }

    fn set_new_app_state(&mut self, newappstate: AppState) {
        let tx = self.background_tx.clone();
        tokio::spawn(async move {
            if tx.send(AppMsg::NewAppState(newappstate)).await.is_err() {
                error!("Tried to update appstate and failed!");
            };
        });
    }

    fn show_editor(&mut self, ctx: egui::Context, filepath: &str) {
        trace!("Showing editor: {}", filepath);

        if self.editor_rename_target.is_empty() {
            self.editor_rename_target = filepath.to_string();
        }
        egui::CentralPanel::default().show(&ctx, |ui| {
            let target_path = PathBuf::from(&self.editor_rename_target);
            let target_path_parent_exists = match target_path.parent() {
                None => false,
                Some(parent_path) => parent_path.exists(),
            };

            ui.horizontal(|ui| {
                let file_label = ui.label("File Path:");

                let filename_editor = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.editor_rename_target)
                            .desired_width(ctx.available_rect().width() * 0.7),
                    ) // 70% of the screen width
                    .labelled_by(file_label.id);

                self.editor_rename_has_focus = filename_editor.has_focus();

                // if they've changed the filename in the box
                if filepath != self.editor_rename_target {
                    if target_path.exists() {
                        ui.label("File already exists!");
                    } else if !target_path_parent_exists {
                        ui.label("Parent path doesn't exist!");
                    } else {
                        filename_editor.ctx.input(|i| {
                            if i.key_pressed(egui::Key::Enter)
                                && filepath != self.editor_rename_target
                            {
                                self.set_new_app_state(AppState::RenameConfirm {
                                    filepath: filepath.to_string(),
                                    newfilepath: self.editor_rename_target.clone(),
                                });
                            }
                        });

                        // show the rename button
                        if ui.button("Rename").clicked() {
                            info!("Clicked rename!");
                            if filepath != self.editor_rename_target {
                                self.app_state = AppState::RenameConfirm {
                                    filepath: filepath.to_string(),
                                    newfilepath: self.editor_rename_target.clone(),
                                };
                            }
                        };
                    }
                }

                if filename_editor.changed() {
                    debug!(
                        "Typed into filename: {} => {}",
                        filepath, self.editor_rename_target
                    );
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Back").clicked() {
                    self.set_new_app_state(AppState::Browser);
                };
                ui.add_space(15.0);
                if ui.button("Delete Image").clicked() {
                    self.set_new_app_state(AppState::DeletePrompt(filepath.to_string()));
                };

                if ui.button("Upload to S3").clicked() {
                    self.set_new_app_state(AppState::UploadPrompt(filepath.to_string()));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Original Path: ");
                ui.label(filepath);
            });
            if let Some(image) = &self.editor_image_cache {
                image.show(ui);
            } else if let Ok(image) = load_image_to_thumbnail(
                &PathBuf::from(filepath),
                Some(Vec2 {
                    x: ui.available_width() * 0.9,
                    y: ui.available_height() * 0.8,
                }),
            ) {
                image.show(ui);
                self.editor_image_cache = Some(image);
            }
        });
    }

    fn show_rename_confirm(&mut self, ctx: egui::Context, filepath: String, newfilename: String) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Please confirm rename");
            });
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(&filepath);
            });
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(&newfilename);
            });
            ui.horizontal(|ui| {
                let confirm =
                    ui.button(RichText::new("Confirm").text_style(egui::TextStyle::Heading));

                let cancel =
                    ui.button(RichText::new("Cancel").text_style(egui::TextStyle::Heading));

                if confirm.clicked() {
                    // rename the file
                    self.do_rename(&ctx, &filepath, &newfilename);
                }

                if cancel.clicked() {
                    self.app_state = AppState::Editor {
                        filepath: filepath.clone(),
                    };
                }
            });
        });
    }
    fn show_delete_prompt(&mut self, ctx: egui::Context, filepath: String) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Please confirm deletion");
            });
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(&filepath);
            });

            ui.horizontal(|ui| {
                let confirm = ui.button("Confirm");

                let cancel = ui.button("Cancel");

                if confirm.clicked() {
                    // rename the file
                    match std::fs::remove_file(&filepath) {
                        Ok(_) => {
                            info!("Deleted {}", filepath);
                            // the browser image list will be wrong at this point, so tell it to cache
                            self.start_update(&ctx);
                            self.app_state = AppState::Browser;
                        }
                        Err(err) => {
                            self.app_state = AppState::ShowError {
                                message: format!("Failed to delete file: {:?}", err),
                                next_state: Some(Box::new(AppState::Editor {
                                    filepath: filepath.clone(),
                                })),
                            };
                        }
                    }
                }

                if cancel.clicked() {
                    self.app_state = AppState::Editor { filepath };
                }
            });
        });
    }
    fn show_upload_prompt(&mut self, ctx: egui::Context, filepath: String) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Confirm upload...");
            });
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(&filepath);
            });

            ui.horizontal(|ui| {
                let confirm = ui.button("Confirm");

                let cancel = ui.button("Cancel");

                if confirm.clicked() {
                    // rename the file

                    debug!("Sending upload message for: {}", filepath);
                    let tx = self.background_tx.clone();
                    let target_filepath = filepath.clone();
                    tokio::spawn(async move {
                        if let Err(err) = tx.send(AppMsg::UploadImage(target_filepath)).await {
                            error!("Failed to send background message: {}", err.to_string());
                        };
                    });
                }

                if cancel.clicked() {
                    self.set_new_app_state(AppState::Editor { filepath });
                }
            });
        });
    }

    fn show_uploading(&mut self, ctx: Context, filepath: String) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Uploading...");
            });
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(filepath);
            });
        });
    }

    fn do_rename(&mut self, ctx: &Context, filepath: &str, newfilename: &str) {
        match std::fs::rename(filepath, newfilename) {
            Ok(_) => {
                debug!("Renamed {} to {}", filepath, newfilename);
                self.start_update(ctx);
                self.app_state = AppState::Editor {
                    filepath: newfilename.to_string(),
                }
            }
            Err(err) => {
                self.app_state = AppState::ShowError {
                    message: format!("Failed to rename file: {:?}", err),
                    next_state: Some(Box::new(AppState::Editor {
                        filepath: filepath.to_string(),
                    })),
                }
            }
        }
    }
}

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
        Box::new(|cc| Box::new(MemeTool::new(cc, foreground_rx, background_tx))),
    )
}
