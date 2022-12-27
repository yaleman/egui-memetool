use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
// use web_sys::window;
use yew::prelude::*;

use memetool_shared::{FileList, ImageData};

const PER_PAGE: u32 = 20;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    /// Allows you to refer to a file on the filesystem, returns an `asset://localhost/<filepath>` url as a `JsValue::String.`
    fn convertFileSrc(filePath: &str, scheme: Option<&str>) -> JsValue;

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Serialize, Deserialize)]
struct PathArgs<'a> {
    pub path: &'a str,
    pub limit: u32,
    pub offset: u32,
}


#[derive(Clone, Properties, PartialEq)]
pub struct ImageProps {
    pub file_path: String,
}

#[function_component(ImageHandler)]
pub fn image_handler(props: &ImageProps ) -> Html {
    html!{ <p>{"Looking at :"} {format!("{}", &props.file_path )} </p>}
}

#[derive(Debug, PartialEq, Eq)]
pub enum Msg {
    ImageHandler{ file_path: String },
    Browser,
    ScrollFirst,
    ScrollLeft,
    ScrollRight,
    GotImages{ files: FileList },
    Event { event: MouseEvent },
    KeyEvent { event: KeyboardEvent },
}


#[derive(Clone, PartialEq)]
pub enum WindowMode {
    Browser,
    ImageHandler{ file_path: String },
}


#[derive(Clone, Properties, PartialEq)]
pub struct BrowserProps {
    #[prop_or("~/Downloads/".to_string())]
    pub file_path: String,
    #[prop_or(0)]
    pub offset: u32,
    #[prop_or(PER_PAGE)]
    pub limit: u32,
    #[prop_or_default]
    pub files_list: Vec<ImageData>,
}

pub struct Browser{
    pub file_path: String,
    pub offset: u32,
    pub limit: u32,
    pub files_list: Vec<ImageData>,
    pub total_files: usize,
    pub window_mode:  WindowMode,
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
        ctx.link().send_future(update_file_list(file_path, ctx.props().offset, ctx.props().limit));

        Browser {
            offset: ctx.props().offset,
            limit: ctx.props().limit,
            file_path: ctx.props().file_path.clone(),
            files_list:  vec![],
            total_files: 0,
            window_mode: WindowMode::Browser,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {

        log(&format!("Got message: {msg:?}"));
        match msg {
            Msg::KeyEvent { event } => {
                log(&format!("Got event: {event:?}"));
                false
            }
            Msg::Event { event } => {
                log(&format!("Got event: {event:?}"));
                false
            }
            Msg::Browser => {
                self.window_mode = WindowMode::Browser;
                true
            },
            Msg::ImageHandler { file_path }  => {
                log(&format!("Got image: {:?}", file_path));
                self.window_mode = WindowMode::ImageHandler { file_path };
                true
            },
            Msg::ScrollLeft => {
                if self.offset >= PER_PAGE {
                    self.offset -= PER_PAGE;
                } else {
                    log(&format!("Already at the start, offset is {}", self.offset));
                }
                self.get_new_files(ctx);
                true
            },
            Msg::ScrollRight => {
                self.offset += PER_PAGE;
                self.get_new_files(ctx);
                true
            },
            Msg::ScrollFirst => {
                self.offset = 0;
                self.get_new_files(ctx);
                true
            },
            Msg::GotImages{ files } => {
                let mut images: Vec<ImageData> = vec![];

                for filepath in files.files.into_iter() {
                    let ic = serde_wasm_bindgen::from_value(convertFileSrc(&filepath, None));
                    if let Ok(ic) = ic {
                        let content_type = match mime_guess::from_path(&ic).first(){
                            Some(val) => val.to_string(),
                            None => String::from("image/jpeg"),
                        };

                        let img = ImageData {
                            filename: ic,
                            content_type,
                        };
                        images.push(img);
                    }
                }
                self.files_list = images;
                self.total_files = files.total_files;
                true
            },
            // _ => false
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        log("View...");

        match self.window_mode.clone() {
            WindowMode::Browser => self.browser_view(&ctx),
            WindowMode::ImageHandler { file_path } => self.imagehandler_view(&ctx, file_path),
        }

    }
}

impl Browser {
    fn get_new_files(&self, ctx: &Context<Self>) {
        ctx.link().send_future(update_file_list(self.file_path.clone(), self.offset, self.limit));
    }

    fn browser_view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <>
            // <main class="container">
                <style type="text/css">
                {"
                .img_block {
                    width: 200px;
                    height: 200px;
                    display: inline-block;
                    vertical-align: middle;
                }"}
                </style>
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
                            self.files_list.clone().into_iter().map(|f| {
                                let file_path = f.filename.clone();
                                html!{
                                    <div class="img_block">
                                        <img
                                            src={f.filename.clone()}
                                            style="max-width: 197px; max-height: 197px;"
                                            alt={f.filename.clone()}
                                            onclick={ctx.link().callback(move |_| {

                                                Msg::ImageHandler{file_path: file_path.to_owned() }
                                            }
                                            )}
                                        />
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                    }
                </ul>
                </div>
            </>
            // </main>
        }
    }

    fn imagehandler_view(&self, ctx: &Context<Self>, file_path: String) -> Html {

        let button = NodeRef::default();

        yew_hooks::use_event(button.clone(), "click", move |_: MouseEvent| {
            log("Clicked!");
        });

        let link = ctx.link().clone();
        yew_hooks::use_event_with_window("onkeyup", move |e: KeyboardEvent| {
            link.callback(move |event| Msg::KeyEvent{event});
            log(&format!("{} is pressed!", e.key()).as_str());
        });


        html!{
            <div>
                <button onclick={ctx.link().callback(move |_| Msg::Browser)}>{"Back"}</button>
                <button onclick={ctx.link().callback(move |event| Msg::Event{event})}>{"Test"}</button>
                <div>{file_path}</div>
            </div>
        }
    }
}

#[function_component(MainApp)]
pub fn main() -> Html {
    html!{
        <Browser />
    }
}


async fn update_file_list(path: String, offset: u32, limit:u32) -> Msg {
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