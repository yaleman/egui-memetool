use gloo::events::EventListener;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::{prelude::*, JsCast};
use yew::prelude::*;

use memetool_shared::{FileList, ImageAction, ImageData, ImagePassed, PathArgs, RESIZE_DEFAULTS};

pub mod imagehandler;

const PER_PAGE: u32 = 20;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    /// Allows you to refer to a file on the filesystem, returns an `asset://localhost/<filepath>` url as a `JsValue::String.`
    fn convertFileSrc(filePath: &str, scheme: Option<&str>) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "fs"])]
    /// Allows you to refer to a file on the filesystem, returns an `asset://localhost/<filepath>` url as a `JsValue::String.`
    fn removeFile(file: &str, args: Option<&str>) -> JsValue;

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

}

#[derive(Clone, Properties, Eq, PartialEq)]
pub struct ImageProps {
    pub file_path: String,
}

#[function_component(ImageHandler)]
pub fn image_handler(props: &ImageProps) -> Html {
    html! { <p>{"Looking at :"} {format!("{}", &props.file_path )} </p>}
}

#[derive(Debug, PartialEq, Eq)]
pub enum Msg {
    ImageLoad {
        image_data: ImagePassed,
    },
    ImageHandler {
        image_data: ImageData,
    },
    ImageAction {
        image_data: ImageData,
        action: ImageAction,
    },
    ShowImageRename {
        image_data: ImageData,
    },
    Browser,
    BrowserReload,
    BrowserNextImage,
    BrowserPrevImage,
    ScrollFirst,
    ScrollLeft,
    ScrollRight,
    GotImages {
        files: FileList,
    },
    // MouseEvent { event: MouseEvent },
    KeyEvent {
        event: KeyboardEvent,
    },
    Error {
        error: String,
    },
}

#[derive(Clone, Eq, PartialEq)]
pub enum WindowMode {
    Browser,
    ImageHandler { image_data: ImageData },
    ImageRenamer {
        image_data: ImageData,
    },
}

#[derive(Clone, Properties, Eq, PartialEq)]
pub struct BrowserProps {
    #[prop_or("~/Downloads/".to_string())]
    pub file_path: String,
    #[prop_or(0)]
    pub offset: u32,
    #[prop_or(PER_PAGE)]
    pub limit: u32,
    #[prop_or_default]
    pub files_list: Vec<ImageData>,
    #[prop_or(0)]
    pub selected_image_offset: u32,
}

pub struct Browser {
    pub file_path: String,
    pub offset: u32,
    pub limit: u32,
    pub files_list: Vec<ImageData>,
    pub total_files: usize,
    pub window_mode: WindowMode,
    /// Holds the keyboard event listener when the renderer's started.
    pub kbd_listener: Option<EventListener>,
    pub selected_image_offset: u32,
}

// pub fn get_value_from_input_event(e: InputEvent) -> String {
//     let event: Event = e.dyn_into().unwrap_throw();
//     let event_target = event.target().unwrap_throw();
//     let target: HtmlInputElement = event_target.dyn_into().unwrap_throw();
//     target.value()
// }

impl Component for Browser {
    type Message = Msg;
    type Properties = BrowserProps;

    fn create(ctx: &Context<Self>) -> Self {
        let file_path = ctx.props().file_path.clone();
        ctx.link().send_future(update_file_list(
            file_path,
            ctx.props().offset,
            ctx.props().limit,
        ));

        Browser {
            offset: ctx.props().offset,
            limit: ctx.props().limit,
            file_path: ctx.props().file_path.clone(),
            files_list: vec![],
            total_files: 0,
            window_mode: WindowMode::Browser,
            kbd_listener: None,
            selected_image_offset: 0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        log(&format!("Got message: {msg:?}"));
        match msg {
            Msg::Error { error } => {
                log(&error);
                false
            }
            Msg::KeyEvent { event } => {
                // log(&format!("Got key event! {:?}", event.key()));
                self.handle_key_event(ctx, event);
                true
            }
            // Msg::MouseEvent { event } => {
            //     log(&format!("Got event: {event:?}"));
            //     log(&format!("Target: {:?}", event.target().unwrap()));

            //     false
            // }
            Msg::Browser => {
                self.window_mode = WindowMode::Browser;
                true
            }
            Msg::BrowserPrevImage => {
                if self.selected_image_offset > 0 {
                    self.selected_image_offset -= 1;
                }
                true
            }
            Msg::BrowserNextImage => {
                if self.selected_image_offset < PER_PAGE - 1 {
                    self.selected_image_offset += 1;
                }
                true
            }

            Msg::BrowserReload => {
                self.get_new_files(ctx);
                self.window_mode = WindowMode::Browser;
                self.selected_image_offset = 0;
                true
            }
            Msg::ImageAction {
                image_data: _,
                action,
            } => {
                log(&format!("Action: {action:?}"));
                false
            }
            Msg::ImageLoad { image_data } => {
                ctx.link()
                    .send_future(load_image_for_imageviewer(image_data));
                false
            }
            Msg::ImageHandler { image_data } => {
                // log(&format!("Got image: {:?}", image_data));
                self.window_mode = WindowMode::ImageHandler { image_data };
                true
            }
            Msg::ScrollLeft => {
                if self.offset >= PER_PAGE {
                    self.offset -= PER_PAGE;
                } else {
                    log(&format!("Already at the start, offset is {}", self.offset));
                }
                self.get_new_files(ctx);
                true
            }
            Msg::ScrollRight => {
                self.offset += PER_PAGE;
                self.get_new_files(ctx);
                true
            }
            Msg::ScrollFirst => {
                self.offset = 0;
                self.get_new_files(ctx);
                true
            }
            Msg::ShowImageRename { image_data } => {
                self.window_mode = WindowMode::ImageRenamer { image_data };
                true
            }
            Msg::GotImages { files } => {
                let mut images: Vec<ImageData> = vec![];

                for filepath in files.files.into_iter() {
                    let file_url = serde_wasm_bindgen::from_value(convertFileSrc(&filepath, None));
                    if let Ok(file_url) = file_url {
                        let content_type = match mime_guess::from_path(&file_url).first() {
                            Some(val) => val.to_string(),
                            None => String::from("image/jpeg"),
                        };

                        let img = ImageData {
                            file_path: filepath,
                            file_url: Some(file_url),
                            content_type,
                            ..ImageData::default()
                        };
                        images.push(img);
                    }
                }
                self.files_list = images;
                self.total_files = files.total_files;
                true
            } // _ => false
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match self.window_mode.clone() {
            WindowMode::Browser => self.browser_view(ctx),
            WindowMode::ImageHandler { image_data } => self.imagehandler_view(ctx, image_data),
            WindowMode::ImageRenamer { image_data } => {
                html!{
                    <imagehandler::ImageRenamer original_path={image_data.file_path} />
                }
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }

        let document = web_sys::window().unwrap().document().unwrap();
        let ct = ctx.link().to_owned();
        let listener = EventListener::new(&document, "keydown", move |event| {
            let event = event
                .dyn_ref::<web_sys::KeyboardEvent>()
                .unwrap_throw()
                .to_owned();
            ct.send_message(Msg::KeyEvent { event });
        });

        self.kbd_listener.replace(listener);
    }
}

impl Browser {
    fn get_new_files(&self, ctx: &Context<Self>) {
        ctx.link().send_future(update_file_list(
            self.file_path.clone(),
            self.offset,
            self.limit,
        ));
    }

    fn browser_view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <>
                <div class="row">
                    if self.offset >= PER_PAGE {
                        <button onclick={ ctx.link().callback(move |_| Msg::ScrollFirst) }>{"First Page"}</button>
                        <button
                            onclick={ ctx.link().callback(move |_| Msg::ScrollLeft) }
                        >{"Previous Page"}</button>
                    }
                    // <input id="file-path" ref={file_path_ref} type="file" webkitdirectory={Some("")} />
                    // <input id="greet-input" ref={file_path_input_ref} placeholder="~/Downloads" value={"~/Downloads/"} />
                    // <button type="button" onclick={file_path_updater}>{"Greet"}</button>
                    <button>{"Total Files:"} {self.total_files.to_string()}</button>
                    <button
                        onclick={ ctx.link().callback(move |_| Msg::ScrollRight) }
                        >{"Next Page"}</button>

                </div>

                <div class="row">
                <ul>
                    {
                        if self.files_list.is_empty() {
                            html!{<p>{ "No files found or could not read dir..." }</p>}
                        } else {
                            let mut images = vec![];
                            for (index, f) in self.files_list.iter().enumerate() {
                            // self.files_list.iter().enumerate().map(|(index, f)| {
                                let image_data = ImagePassed {
                                    path: f.file_path.clone(),
                                    file_url: f.file_url.clone().unwrap(),
                                    image_format: f.file_type.clone(),
                                };
                                let img_class = match index as u32 == self.selected_image_offset {
                                    true => "img_block_selected",
                                    false => "img_block",
                                };
                                images.push(
                                    html!{
                                        <li class="imagelist">
                                        <div class={img_class}>
                                            <center>
                                            <img
                                                src={f.file_url.clone()}
                                                class="img_block"
                                                alt={f.file_path.clone()}
                                                onclick={
                                                ctx.link().callback(move |_| {
                                                    Msg::ImageLoad { image_data: image_data.to_owned() }
                                                })}

                                            /></center>
                                        </div>
                                        </li>
                                    });
                            }
                            html!{images.into_iter().collect::<Html>()}
                        }
                    }
                </ul>
                </div>
            </>
        }
    }

    fn imagehandler_view(&self, ctx: &Context<Self>, image_data: ImageData) -> Html {
        // log(&format!("Image_info: {image_info:?}"));

        let dimension_data = match image_data.file_dimensions {
            Some(val) => {
                let (x, y) = val;
                html! {<p>{"Original image dimensions: "}{x}{"x"}{y}</p>}
            }
            None => html! {<></>},
        };
        let filename_data = html! {
            <p>{"Filename: "}{image_data.file_path.clone()}</p>
        };
        html! {
            <>
            <div class="row">
                <button autofocus=true onclick={ctx.link().callback(move |_| Msg::Browser)}>{"Back"}</button>
                // <button onclick={ctx.link().callback(move |event| Msg::MouseEvent{event})}>{"Test"}</button>
            </div>
            // TODO: add image data, file size, width/height etc.
            <div class="row">
                <div class="col imageHandlerCol">
                    <img
                    src={image_data.file_url.clone()}
                    style="max-width: 100%; max-height: 100%;"
                    alt={image_data.file_path}
                    // onclick={ctx.link().callback(move |_| {
                    //     Msg::ImageHandler{file_path: file_path.to_owned() }
                    // }
                    // )}
                />
                </div>
                <div class="col">
                    {dimension_data}
                    {filename_data}

                    <h3>{"Available actions:"}</h3>
                    <ul>
                    <li>{"r - Rename"}</li>
                    <li>{"s - reSize"}</li>
                    </ul>
                </div>
            </div>

            </>
        }
    }

    fn handle_key_event(&self, ctx: &Context<Self>, key_event: KeyboardEvent) {
        match &self.window_mode {
            WindowMode::Browser => match key_event.key().as_str() {
                "PageUp" => ctx.link().send_message(Msg::ScrollLeft),
                "PageDown" => ctx.link().send_message(Msg::ScrollRight),
                "ArrowLeft" => ctx.link().send_message(Msg::BrowserPrevImage),
                "ArrowRight" => ctx.link().send_message(Msg::BrowserNextImage),
                "Enter" => {
                    let image_data = self
                        .files_list
                        .get(self.selected_image_offset as usize)
                        .unwrap()
                        .to_owned();
                    ctx.link().send_message(Msg::ImageHandler { image_data })
                }
                "Home" => ctx.link().send_message(Msg::ScrollFirst),
                _ => {
                    log(&format!(
                        "Key event in browser, no action required. Pressed: {:?})",
                        key_event.key(),
                    ));
                }
            },
            WindowMode::ImageHandler { image_data } => {
                let image_data = image_data.to_owned();
                match key_event.key().as_str() {
                    "Escape" => {
                        ctx.link().send_message(Msg::Browser);
                    }
                    "d" => {
                        log("delete image time!");
                        ctx.link().send_future(
                            delete_image(image_data)
                        );
                    },
                    "r" => {
                        log("r!");
                        ctx.link().send_message(Msg::ShowImageRename { image_data })
                    },
                    "s" => {
                        log("reSizing!");
                        ctx.link().send_message(Msg::ImageAction {
                            image_data,
                            action: ImageAction::Resize{ x: RESIZE_DEFAULTS.0, y: RESIZE_DEFAULTS.0 },
                        })
                    },
                    "S" => {
                        log("we should pop a thing prompting for a size here...");
                    }
                    _ => log(&format!(
                        "Key event in ImageHandler({image_data:?}), no action required. Pressed: {:?}",
                        key_event.key()
                    ))
                }
            },
            WindowMode::ImageRenamer { image_data } => {
                match key_event.key().as_str() {
                    "Escape" => {
                        ctx.link().send_message(Msg::ImageHandler { image_data: image_data.to_owned() });
                    }
                    _ => log(&format!(
                        "Key event in ImageHandler({image_data:?}), no action required. Pressed: {:?}",
                        key_event.key()
                    ))
                }

            }
        }
    }
}

#[function_component(MainApp)]
pub fn main() -> Html {
    html! {
        <Browser />
    }
}

#[derive(Serialize, Deserialize)]
struct PassIt {
    imagedata: ImagePassed,
}

async fn delete_image(image_data: ImageData) -> Msg {
    let result = invoke(
        "delete_image",
        to_value(&PassIt {
            imagedata: (&image_data).into(),
        })
        .unwrap(),
    )
    .await;
    let result: bool = serde_wasm_bindgen::from_value(result).unwrap_or(false);
    match result {
        true => {
            log("Deleting!");
            let res = removeFile(&image_data.file_path, None);
            log(&format!("File delete result: {res:?}"));
            Msg::BrowserReload
        }
        false => {
            log("NOT Deleting!");
            Msg::ImageHandler {
                image_data: image_data.clone(),
            }
        }
    }
}

async fn load_image_for_imageviewer(image_data: ImagePassed) -> Msg {
    let image_response = invoke(
        "get_image",
        to_value(&PassIt {
            imagedata: image_data.clone(),
        })
        .unwrap(),
    )
    .await;
    let image_data: ImageData = match serde_wasm_bindgen::from_value(image_response) {
        Ok(val) => val,
        Err(err) => {
            return Msg::Error {
                error: format!("Failed to get image data for {}: {err:?}", image_data.path),
            }
        }
    };
    Msg::ImageHandler { image_data }
}

async fn update_file_list(path: String, offset: u32, limit: u32) -> Msg {
    log("Grabbing files...");
    let file_list = invoke(
        "list_directory",
        to_value(&PathArgs {
            path: &path,
            limit,
            offset,
        })
        .unwrap(),
    )
    .await;

    let files: FileList = serde_wasm_bindgen::from_value(file_list).unwrap();
    log("Sending file list...");
    Msg::GotImages { files }
}
