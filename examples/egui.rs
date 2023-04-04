
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#[macro_use]
extern crate lazy_static;

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, TextStyle, RichText};
use eframe::epaint::{FontFamily, FontId, Vec2};
use egui_extras::RetainedImage;
use poll_promise::Promise;
use rayon::prelude::*;


lazy_static!{
    static ref OK_EXTENSIONS: Vec<&'static str> = vec![
        "jpg",
        "gif",
        "png",
        "jpeg",
    ];

    static ref THUMBNAIL_SIZE: Vec2 = Vec2{x: 120.0, y: 100.0};
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
}


struct MemeTool {
    /// Current working directory
    pub workdir: String,
    pub files_list: Vec<PathBuf>,
    pub current_page: usize,
    pub app_state: AppState,
    pub last_checked: Option<String>,
    pub per_page: usize,
    pub browser_images: Vec<Arc<RetainedImage>>,
    pub promise: Option<Promise<Box<MemeTool>>>,
}

impl Default for MemeTool {
    fn default() -> Self {
        Self {
            workdir: "~/Downloads".into(),
            files_list: vec![],
            current_page: 0,
            app_state: AppState::Browser,
            last_checked: None,
            per_page: 20,
            browser_images: vec![],
            promise: None,
        }
    }
}

impl eframe::App for MemeTool {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match self.app_state {
            AppState::Browser => self.show_browser(&ctx),
        }

    }
}


fn threadout(
    workdir: String,
    current_page: usize,
    on_done: Box<dyn FnOnce(Box<MemeTool>) + Send>,
) {
    std::thread::Builder::new()
        .name("ehttp".to_owned())
        .spawn(move || on_done(update_files_list(workdir, current_page)))
        .expect("Failed to spawn ehttp thread");
}

impl MemeTool {

    /// sets some things up
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_text_styles(&cc.egui_ctx);
        Self::default()

    }

    /// Get a given page of file results
    fn get_page(&self) -> Vec<PathBuf> {
        if self.files_list.len() <= self.per_page {
            self.files_list.clone()
        } else {
            match self.files_list.chunks(self.per_page).nth(self.current_page) {
                Some(list) => list.to_vec(),
                None => vec![]
            }
        }
    }


    /// build a threaded promisey thing to update images in the backend.
    fn start_update(&mut self, ctx: &egui::Context) {
        self.browser_images = vec![];
        eprintln!("Starting update in thread...");
        let ctx = ctx.clone();
        let (sender, promise) = Promise::new();
        threadout(self.workdir.clone(), self.current_page, Box::new(move |result| {
            sender.send(result);
            ctx.request_repaint();
        }));
        self.promise = Some(promise);
    }

    fn show_browser(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match &self.last_checked {
                Some(val) => {
                    if val != &self.workdir {
                        println!("{} != {}", val, self.workdir);
                        self.start_update(&ctx)
                    }
                },
                None => {
                    self.start_update(&ctx);
                }
            };
            self.last_checked = Some(self.workdir.clone());

            ui.horizontal(|ui| {
                let name_label = ui.label(RichText::new("Current workdir: ").text_style(heading3()).strong());
                ui.text_edit_singleline(&mut self.workdir)
                    .labelled_by(name_label.id);
            });

            ui.add_space(15.0);
            ui.horizontal(|ui| {
                if ui.button("Prev Page").clicked() {
                    eprintln!("Pref page clicked");
                    if self.current_page > 0 {
                        self.current_page -=1 ;
                        self.last_checked = None;
                    }
                }
                ui.add_space(15.0);
                if ui.button("Next Page").clicked() {
                    eprintln!("Next page clicked");
                    if self.current_page < (self.files_list.len() / self.per_page) {
                        self.current_page +=1 ;
                        self.last_checked = None;
                    }
                }
            });
            ui.add_space(15.0);

            if let Some(promise) = &self.promise {
                // we got an update back!
                if let Some(result) = promise.ready() {
                    self.last_checked = result.last_checked.to_owned();
                    self.browser_images = result.browser_images.to_owned();
                    self.files_list = result.files_list.to_owned();
                    self.promise = None;
                } else {
                    ui.label("Updating...");
                };
            } else {
                // ui.label("Updating...");
            }

            egui::Grid::new("some_unique_id")
                .num_columns(5)
                .show(ui, |ui| {
                let mut col = 0;

                for image in &self.browser_images {
                    image.show_max_size(ui, *THUMBNAIL_SIZE);
                    col += 1;
                    if col > 4 {
                        col = 0;
                        ui.end_row();
                    }
                }
            });

            ui.add_space(15.0);


            ui.horizontal(|ui| {
                ui.label(format!("Number of files: {}", self.files_list.len()));
                let last_checked = &self.last_checked.as_ref();
                ui.label(format!("Last Checked: {}", &last_checked.unwrap_or(&"".to_string())));
                ui.label(format!("Current page: {}", self.current_page))
            });

        });
    }


}

fn load_image_to_thumbnail(filename: &PathBuf) -> Result<RetainedImage, String> {
    eprintln!("Loading {}", filename.to_string_lossy());
    let mut f = File::open(filename).map_err(|err| err.to_string())?;
    let mut buffer = Vec::new();

    // read up to 10 bytes
    f.read(&mut buffer).map_err(|err| err.to_string())?;

    let mut buffer = Vec::new();
    // read the whole file
    f.read_to_end(&mut buffer).map_err(|err| err.to_string())?;

    let image = match egui_extras::RetainedImage::from_image_bytes(
        filename.to_string_lossy(),
        &buffer,
    ) {
        Ok(val) => val,
        Err(err) => return Err(err),
    };
    Ok(image)
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        decorated: true,
        // drag_and_drop_support: todo!(),
        // icon_data: Some(IconData::from("hello world")),
        // initial_window_pos: todo!(),
        // min_window_size: todo!(),
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
        follow_system_theme:true,
        // default_theme: todo!(),
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

fn update_files_list(workdir: String, current_page: usize) -> Box<MemeTool> {
    eprintln!("***** UPDATING FILES LIST from {} *****", workdir);
    let resolvedpath = shellexpand::tilde(&workdir);

    let files_list = match std::fs::read_dir(resolvedpath.to_string()) {
        Ok(dirlist) => {
            dirlist.filter_map(|filename| {
                match filename {
                    Ok(val) => {
                        let pathstr = val.path().clone();
                        let pathstr = pathstr.to_string_lossy().to_lowercase();
                        if OK_EXTENSIONS.iter().any(|ext| {
                            pathstr.ends_with(&format!(".{ext}"))
                        }) {
                            Some(val.path().to_path_buf().to_owned())

                        } else {None}

                    },
                    Err(_) => None
                }
            }).collect()
        },
        Err(_) => vec![],
    };
    let mut memetool = MemeTool{
        current_page,
        workdir: workdir.clone(),
        last_checked: Some(workdir),
        files_list,
        ..Default::default()
    };

    memetool.browser_images = memetool.get_page().par_iter().filter_map(|filepath| {
        match load_image_to_thumbnail(&filepath) {
            Ok(val) => Some(Arc::new(val)),
            Err(err) => {
                eprintln!("Failed to load {}: {}", filepath.to_string_lossy(), err);
                None
            },
        }

    }).collect();
    Box::new(memetool)
}