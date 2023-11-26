use std::collections::HashMap;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use config::Configuration;
use eframe::egui::{self, Context, Grid, Key, RichText, TextureOptions};
use eframe::epaint::{vec2, Vec2};
use egui_extras::RetainedImage;
use image_utils::load_image_from_memory;
use itertools::Itertools;
use log::*;
use text::{configure_text_styles, heading3};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::image_utils::load_image_to_thumbnail;

#[macro_use]
extern crate lazy_static;

pub mod background;
pub mod config;
pub mod image_utils;
pub mod s3_upload;
pub mod text;

lazy_static! {
    pub static ref OK_EXTENSIONS: Vec<&'static str> = vec!["jpg", "gif", "png", "jpeg",];
    pub static ref PER_PAGE: usize = 20;
    pub static ref GRID_X: u8 = 5;
    pub static ref GRID_Y: u8 = 4;
    pub static ref GRID_SPACING: Vec2 = Vec2 { x: 10.0, y: 10.0 };
    pub static ref THUMBNAIL_SIZE: Vec2 = Vec2 { x: 200.0, y: 150.0 };
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
    Configuration,
}

#[derive(Debug)]
pub enum AppMsg {
    LoadImage(ThumbImageMsg),
    ThumbImageResponse(ThumbImageMsg),
    ImageLoadFailed { filename: String, error: String },
    NewAppState(AppState),
    Echo(String),
    UploadImage(String),
    UploadAborted(String),
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

pub struct MemeTool {
    /// Current working directory
    pub workdir: String,
    /// Used in the browser to filter the list of files
    pub search_box: String,
    pub search_box_last: Option<String>,
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
    key_buffer: Vec<egui::Key>,
    editor_image_cache: Option<RetainedImage>,
    editor_rename_target: String,
    editor_rename_has_focus: bool,
    configuration: Option<Configuration>,
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
                AppMsg::Error(message) => {
                    self.app_state = AppState::ShowError {
                        message,
                        next_state: None,
                    }
                }
                AppMsg::UploadAborted(message) => {
                    self.app_state = AppState::ShowError {
                        message,
                        next_state: None,
                    }
                }
            }
        }
        ctx.request_repaint_after(Duration::from_micros(100));

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
            AppState::Configuration => self.show_config(ctx.clone()),
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
    pub fn new(
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
            search_box: "".into(),
            search_box_last: None,
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
            configuration: None,
        }
    }

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
                            AppState::Browser => {
                                self.search_box = "".into();
                            }
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
                            AppState::Configuration => {
                                debug!("User hit escape in config...");
                                // TODO: save config here
                                self.app_state = AppState::Browser;
                            }
                            _ => {}
                        },
                        Key::ArrowLeft => {
                            if let AppState::Browser = self.app_state {
                                self.browser_prev_page();
                            }
                        }
                        Key::ArrowRight => {
                            if let AppState::Browser = self.app_state {
                                self.browser_next_page();
                            }
                        }

                        // Key::A => todo!(),
                        // Key::F1 => todo!(),
                        _ => {
                            // debug!("Unhandled key: {:?}", key);
                        }
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

    /// returns a list of files in the current working directory
    fn read_workdir(&self) -> Vec<PathBuf> {
        let resolvedpath = shellexpand::tilde(&self.workdir);
        match std::fs::read_dir(resolvedpath.to_string()) {
            Ok(dirlist) => dirlist
                .sorted_by_key(|d| {
                    d.as_ref()
                        .unwrap()
                        .file_name()
                        .into_string()
                        .unwrap_or("".into()) // if this fails we're having a *really* bad day.
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
                            debug!("Skipping {} due to extension", pathstr);
                            None
                        }
                    }
                    Err(_) => None,
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    fn update_files_list(&mut self) {
        self.files_list = self.read_workdir();

        let cached_files: Vec<String> = self.browser_images.keys().map(|k| k.to_owned()).collect();

        // clear out the cached_Files that are no longer in the files_list
        for filename in cached_files {
            let filepath = PathBuf::from(&filename);
            if !self.files_list.contains(&filepath) {
                info!("Removing {} from cached files", filename);
                self.browser_images.remove(&filename);
            }
        }

        // after we've cleaned up the cache filter based on search
        if !self.search_box.trim().is_empty() {
            let search_terms: Vec<String> = self
                .search_box
                .trim()
                .split(' ')
                .map(str::to_lowercase)
                .collect();
            self.files_list = self
                .files_list
                .iter()
                .filter_map(|filepath| {
                    let filename = filepath
                        .file_name()
                        .expect("Failed to parse filename from OsStr to String")
                        .to_string_lossy() // if you're doing bad things with file paths then too bad
                        .to_lowercase();
                    if search_terms.iter().all(|term| filename.contains(term)) {
                        Some(filepath.clone())
                    } else {
                        None
                    }
                })
                .collect();
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
            self.sendmessage(AppMsg::LoadImage(ThumbImageMsg {
                filepath: filepath.display().to_string(),
                page: current_page,
                image: None,
            }));
        });
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn check_needs_update(&mut self, ctx: &egui::Context) {
        if let Some(last_box) = self.search_box_last.clone() {
            if last_box != self.search_box {
                debug!("Search box changed to '{}', updating.", self.search_box);
                self.start_update(ctx);
            }
        } else if self.search_box_last.is_none() {
            debug!("Search box changed to '{}', updating.", self.search_box);
            self.start_update(ctx);
        } else {
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
        };
        self.search_box_last = Some(self.search_box.clone());
        self.last_checked_dir = Some(self.workdir.clone());
        self.last_checked_page = Some(self.current_page);
    }

    fn show_browser(&mut self, ctx: egui::Context) {
        // println!("starting show_browser repaint");
        egui::CentralPanel::default().show(&ctx, |ui| {
            self.check_needs_update(&ctx);

            // ui.horizontal(|ui| {
            //     let name_label = ui.label(
            //         RichText::new("Current workdir: ")
            //             .text_style(heading3())
            //             .strong(),
            //     );
            //     ui.text_edit_singleline(&mut self.workdir)
            //         .labelled_by(name_label.id);
            // });

            // search box
            ui.horizontal(|ui| {
                let search_label =
                    ui.label(RichText::new("Search:").text_style(heading3()).strong());
                ui.text_edit_singleline(&mut self.search_box)
                    .labelled_by(search_label.id);
                if ui.button("Reset").clicked() {
                    self.search_box = "".to_string();
                }
            });

            // navigation bars
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                if self.current_page > 0 {
                    if ui.button("First Page").clicked() {
                        self.browser_first_page();
                    };

                    if ui.button("Prev Page").clicked() {
                        self.browser_prev_page();
                    }
                    ui.add_space(15.0);
                }

                if ui.button("Next Page").clicked() {
                    self.browser_next_page();
                }
                #[cfg(debug_assertions)]
                if ui.button("Refresh").clicked() {
                    debug!("Refresh clicked");
                    self.search_box_last = None;
                    self.sendmessage(AppMsg::NewAppState(AppState::Browser));
                }
            });
            ui.add_space(15.0);

            let mut loaded_images = 0;

            Grid::new("browser")
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
                                    self.loading_image.,
                                    // vec2(THUMBNAIL_SIZE.y, THUMBNAIL_SIZE.y),
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
                if ui.button("Configuration").clicked() {
                    self.app_state = AppState::Configuration;
                }

                ui.label(format!("Number of files: {}", self.files_list.len()));
                if let Some(last_checked) = &self.last_checked_dir {
                    ui.label(format!("Last Checked: {}", last_checked));
                };
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
        self.sendmessage(AppMsg::NewAppState(newappstate))
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
                if ui
                    .button(RichText::new("Back").text_style(heading3()))
                    .clicked()
                {
                    self.set_new_app_state(AppState::Browser);
                };
                ui.add_space(15.0);
                if ui
                    .button(RichText::new("Delete Image").text_style(heading3()))
                    .clicked()
                {
                    self.set_new_app_state(AppState::DeletePrompt(filepath.to_string()));
                };

                if ui
                    .button(RichText::new("Upload to S3").text_style(heading3()))
                    .clicked()
                {
                    self.set_new_app_state(AppState::UploadPrompt(filepath.to_string()));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Original Path: ");
                ui.label(filepath);
            });

            let mut image_width = 0;
            let mut image_height = 0;

            if let Some(image) = &self.editor_image_cache {
                image_height = image.height();
                image_width = image.width();
                image.show(ui);
            } else if let Ok(image) = load_image_to_thumbnail(
                &PathBuf::from(filepath),
                Some(Vec2 {
                    x: ui.available_width() * 0.9,
                    y: ui.available_height() * 0.8,
                }),
            ) {
                image_height = image.height();
                image_width = image.width();
                image.show(ui);
                self.editor_image_cache = Some(image);
            }
            ui.label(format!("Image Size: {}x{}", image_width, image_height));

            // show filepath size on disk
            if let Ok(metadata) = std::fs::metadata(filepath) {
                ui.label(format!(
                    "File Size: {}",
                    humansize::format_size(metadata.len(), humansize::DECIMAL)
                ));
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
                if ui
                    .button(RichText::new("Confirm").text_style(heading3()))
                    .clicked()
                {
                    // rename the file
                    debug!("Sending upload message for: {}", filepath);
                    let target_filepath = filepath.clone();
                    self.sendmessage(AppMsg::UploadImage(target_filepath));
                }

                if ui
                    .button(RichText::new("Cancel").text_style(heading3()))
                    .clicked()
                {
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

    /// config UI
    fn show_config(&mut self, ctx: Context) {
        // load config file
        if self.configuration.is_none() {
            self.configuration = match Configuration::try_new() {
                Ok(val) => Some(val),
                Err(err) => {
                    self.app_state = AppState::ShowError {
                        message: format!("Failed to load configuration: {:?}", err),
                        next_state: Some(Box::new(AppState::Browser)),
                    };
                    return;
                }
            }
        }
        let mut endpoint_url = String::new();

        if let Some(config) = &self.configuration.as_ref().unwrap().s3_endpoint {
            endpoint_url = config.to_owned();
        };

        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading(RichText::new("Configuration").text_style(heading3()));
            });
            ui.horizontal(|ui| {
                // TODO: need to save config here
                if ui.button("Back").clicked() {
                    self.app_state = AppState::Browser;
                    if let Some(config) = self.configuration.as_mut() {
                        if let Err(err) = config.save() {
                            self.app_state = AppState::ShowError {
                                message: format!("Failed to save configuration: {:?}", err),
                                next_state: Some(Box::new(AppState::Browser)),
                            };
                        }
                    }
                }
            });

            ui.heading("S3 Configuration");
            Grid::new("config_grid")
                .striped(true)
                .min_col_width(100.0)
                .spacing([10.0, 10.0])
                .num_columns(2)
                .show(ui, |ui| {
                    let s3_access_key_id_label = ui.label("S3 Access Key ID");
                    ui.add(
                        egui::TextEdit::singleline(
                            &mut self.configuration.as_mut().unwrap().s3_access_key_id,
                        )
                        .desired_width(ctx.available_rect().width() * 0.7),
                    )
                    .labelled_by(s3_access_key_id_label.id);
                    ui.end_row();

                    let s3_secret_access_key_label = ui.label("S3 Secret");
                    ui.add(
                        egui::TextEdit::singleline(
                            &mut self.configuration.as_mut().unwrap().s3_secret_access_key,
                        )
                        .password(true)
                        .desired_width(ctx.available_rect().width() * 0.7),
                    )
                    .labelled_by(s3_secret_access_key_label.id);
                    ui.end_row();

                    let bucket_label = ui.label("S3 Bucket");
                    ui.add(
                        egui::TextEdit::singleline(
                            &mut self.configuration.as_mut().unwrap().s3_bucket,
                        )
                        .desired_width(ctx.available_rect().width() * 0.7),
                    )
                    .labelled_by(bucket_label.id);
                    ui.end_row();

                    let region_label = ui.label("S3 Region");
                    ui.add(
                        egui::TextEdit::singleline(
                            &mut self.configuration.as_mut().unwrap().s3_region,
                        )
                        .desired_width(ctx.available_rect().width() * 0.7),
                    )
                    .labelled_by(region_label.id);
                    ui.end_row();

                    let endpoint_label = ui.label("S3 Endpoint");
                    let endpoint = ui
                        .add(
                            egui::TextEdit::singleline(&mut endpoint_url)
                                .desired_width(ctx.available_rect().width() * 0.7),
                        )
                        .labelled_by(endpoint_label.id);
                    // update the internal state
                    if endpoint.changed() {
                        self.configuration.as_mut().unwrap().s3_endpoint =
                            Some(endpoint_url.clone());
                    }
                    ui.end_row();
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

    /// force-update the browser view
    fn browser_new_page(&mut self) {
        self.search_box_last = None;
        self.last_checked_dir = None;
        self.sendmessage(AppMsg::NewAppState(AppState::Browser));
    }

    /// take you to the previous page
    fn browser_prev_page(&mut self) {
        debug!("Prev page clicked");
        if self.current_page > 0 {
            self.current_page -= 1;
        }
        self.browser_new_page();
    }

    /// take you to the next page
    fn browser_next_page(&mut self) {
        debug!("Next page clicked");
        if self.current_page < (self.files_list.len() / self.per_page) {
            self.current_page += 1;
        } else {
            if self.current_page * self.per_page > self.files_list.len() {
                error!(
                    "Current page={} Per page={} Files list len={}",
                    self.current_page,
                    self.per_page,
                    self.files_list.len()
                );
            }
            error!("Uh, too far bruh!");
        }
        self.browser_new_page();
    }

    /// take you to the first page
    fn browser_first_page(&mut self) {
        debug!("First page clicked");
        self.current_page = 0;
        self.browser_new_page();
    }

    /// send a message using the internal broadcast channel
    fn sendmessage(&mut self, msg: AppMsg) {
        let tx = self.background_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = tx.send(msg).await {
                error!("Failed to send background message: {}", err.to_string());
            };
        });
    }
}
