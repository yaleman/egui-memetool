use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use memetool_shared::FileList;


#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}


#[derive(Serialize, Deserialize)]
struct PathArgs<'a> {
    path: &'a str,
}



#[function_component(App)]
pub fn app() -> Html {
    let greet_input_ref = use_node_ref();
    let file_path_ref = use_node_ref();


    let file_path = use_state(|| String::new());
    let files_list: UseStateHandle<Vec<String>> = use_state(|| vec![] );


    {
        let files_list = files_list.clone();

        let file_path = file_path.clone();
        let file_path2 = file_path.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
                    let file_list = invoke(
                        "list_directory",
                        to_value(&PathArgs { path: &*file_path }).unwrap(),
                    )
                    .await;
                    let file_list: FileList = serde_wasm_bindgen::from_value(file_list).unwrap();
                    files_list.set(file_list.files);
                });

                || {}
            },
            file_path2,
        );
    }

    let greet = {
        let file_path = file_path.clone();
        let greet_input_ref = greet_input_ref.clone();
        Callback::from(move |_| {
            file_path.set(greet_input_ref.cast::<web_sys::HtmlInputElement>().unwrap().value());
        })
    };

    html! {
        <main class="container">

            <div class="row">
                <input id="file-path" ref={file_path_ref} type="file" webkitdirectory={Some("")} />
                <input id="greet-input" ref={greet_input_ref} placeholder="Enter a name..." />
                <button type="button" onclick={greet}>{"Greet"}</button>
            </div>

            <div class="row">
            <ul>
                {
                    if files_list.is_empty() {
                        html!{<p>{ "No files found or could not read dir..." }</p>}
                    } else {
                        files_list.iter().map(|f| {
                            html!{
                                <li>
                                    <img src={format!("file://{f}")} width="200" height="200" />
                                    <br />{ format!("{:?}", f) } </li>
                            }
                        }).collect::<Html>()
                    }
                }
            </ul>
            </div>
        </main>
    }
}
