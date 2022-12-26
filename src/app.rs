use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
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

pub enum Msg {
    ImageHandler{ file_path: String },
    Browser,
}


#[derive(Clone, Properties, PartialEq)]
pub struct BrowserProps {
    pub file_path: String,
    pub offset: u32,
    pub limit: u32,

}


#[function_component(Browser)]
pub fn browser(props: &BrowserProps) -> Html {
    let file_path_input_ref = use_node_ref();

    #[allow(clippy::redundant_closure)]
    let file_path = use_state(|| props.file_path.clone());
    #[allow(clippy::redundant_closure)]
    let files_list: UseStateHandle<Vec<ImageData>> = use_state(|| Vec::new());

    let offset = use_state(|| props.offset);
    let limit = use_state(|| props.limit);
    #[allow(clippy::redundant_closure)]
    let total_files = use_state(|| usize::default());

    {
        let files_list = files_list.clone();

        let file_path = file_path.clone();
        let file_path2 = file_path.clone();

        let total_files = total_files.clone();
        let offset = offset.clone();
        let offset2 = offset.clone();

        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
                    let file_list = invoke(
                        "list_directory",
                        to_value(&PathArgs {
                            path: &file_path,
                            limit: *limit,
                            offset: *offset,
                        })
                        .unwrap(),
                    )
                    .await;

                    let file_list: FileList = serde_wasm_bindgen::from_value(file_list).unwrap();
                    total_files.set(file_list.total_files);
                    let mut images: Vec<ImageData> = vec![];

                    for filepath in file_list.files.into_iter() {
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

                    files_list.set(images);
                });

                || {}
            },
            (file_path2, offset2),
        );
    }

    let file_path_updater = {
        let file_path = file_path.clone();
        let file_path_input_ref = file_path_input_ref.clone();
        Callback::from(move |_| {
            file_path.set(file_path_input_ref
                    .cast::<web_sys::HtmlInputElement>()
                    .unwrap()
                    .value()
            );
        })
    };

    let scroll_first: Callback<MouseEvent> = {
        let offset = offset.clone();
        Callback::from(move |_| {
            offset.set(0);
        })
    };

    let scroll_left: Callback<MouseEvent> = {
        let offset = offset.clone();
        let new_offset: u32 = if props.offset > PER_PAGE {
            props.offset - PER_PAGE
        } else {
            PER_PAGE
        };
        Callback::from(move |_| {
            offset.set(new_offset);
        })
    };
    let scroll_right: Callback<MouseEvent> = {
        let offset = offset.clone();
        let new_offset = *offset + PER_PAGE;
        Callback::from(move |_| {
            offset.set(new_offset);
        })
    };

    // let select_image: Callback<MouseEvent> = {
    //     Callback::from(move |event: MouseEvent| {
    //         log(&format!("got mouse event: {:?}", event));
    //         if let Some(target) = event.target() {
    //             log(&format!("target: {:?}", target))
    //         }
    //     })
    // };

    html! {
        <main class="container">
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
                if *offset >= PER_PAGE {
                    <button onclick={scroll_first}>{"First Page"}</button>
                    <button onclick={scroll_left}>{"Previous Page"}</button>
                }
                // <input id="file-path" ref={file_path_ref} type="file" webkitdirectory={Some("")} />
                <input id="greet-input" ref={file_path_input_ref} placeholder="~/Downloads" value={"~/Downloads/"} />
                <button type="button" onclick={file_path_updater}>{"Greet"}</button>
                <button>{"Total Files:"} {total_files.to_string()}</button>
                <button onclick={scroll_right}>{"Next Page"}</button>

            </div>

            <div class="row">
            <ul>
                {
                    if files_list.is_empty() {
                        html!{<p>{ "No files found or could not read dir..." }</p>}
                    } else {
                        files_list.iter().map(|f| {
                            html!{
                                <div class="img_block">
                                    <img
                                        src={f.filename.clone()}
                                        style="max-width: 197px; max-height: 197px;"
                                        alt={f.filename.clone()}
                                        // onclick={ctx.link().callback(|_| Msg::ImageHandler{file_path: f.filename.clone()})}
                                    />
                                </div>
                            }
                        }).collect::<Html>()
                    }
                }
            </ul>
            </div>

        </main>
    }
}

#[function_component(MainApp)]
pub fn main() -> Html {
    html!{
        <Browser offset=0 limit={PER_PAGE} file_path="~/Downloads" />
    }
}