use web_sys::HtmlInputElement;
use yew::prelude::*;
use wasm_bindgen::JsCast;

use super::log;

#[derive(Eq, PartialEq, Properties)]
pub struct ImageRenamerProps {
    pub original_path: String,
}

pub struct ImageRenamer {
    pub original_path: String,
    pub new_filename: Option<String>,
}

#[derive(Debug)]
pub enum ImageRenamerMsg {
    // Start,
    Commit { new_filename: String },
    FilenameUpdated { new_filename: String },
    // Cancelled,
}

impl Component for ImageRenamer{
    type Message = ImageRenamerMsg;
    type Properties = ImageRenamerProps;

    fn create(ctx: &Context<Self>) -> Self {
        ImageRenamer {
            original_path: ctx.props().original_path.to_owned(),
            new_filename: None,
        }
    }


    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        log(&format!("ImageRenamer RX message: {msg:?}"));
        match msg {
            ImageRenamerMsg::Commit { new_filename } => {
                log(&format!("Renaming to {new_filename}"));
            },
            ImageRenamerMsg::FilenameUpdated { new_filename } => {
                self.new_filename = Some(new_filename);
            }
        }
        true

    }

    fn view(&self, ctx: &Context<Self>) -> Html {

        let file_name = match &self.new_filename {
            Some(new_path) => new_path,
            None => {
                self.original_path.as_str().split("/").last().unwrap()
            }
        };
        let file_name = file_name.to_string();

        let new_path = match self.new_filename.clone() {
            Some(new_name) => html!{<p>{"to "}{new_name}</p>},
            None => html!{<></>}
        };


        html!{
            <div class="imageRenamerBody">
            <form action="" method="GET" onsubmit={
                ctx.link().callback(move |e: SubmitEvent| {
                    e.prevent_default(); // block navigating on submit
                    log(&format!("{:?}", e));

                    log(&format!("Event target: {:?}", e.event_target()));

                    ImageRenamerMsg::Commit{ new_filename: "asdfasdf".to_string() }
            })
            }>
            <table cellpadding="3" cellspacing="0" width="100%">
                <tr>
                    <td class="col">{"Original path: "}</td>
                    <td class="col">{self.original_path.clone()}</td>
                </tr>
                <tr>
                    <td class="col">{"New Path: "}</td>
                    <td class="col min-width: 80%">
                        <input
                            type="text"
                            name="new_path"
                            style="min-width: 80%; max-width: 100%"
                            value={file_name}
                            oninput={ ctx.link().callback(move |e: InputEvent| {
                                let event: Event = e.dyn_into().unwrap();
                                let event_target = event.target().unwrap();
                                let target: HtmlInputElement = event_target.dyn_into().unwrap();
                                ImageRenamerMsg::FilenameUpdated{ new_filename: target.value() }
                            })}/>
                        </td>
                </tr>
                <tr>
                    <td class="col">{" "}</td>
                    <td class="col"><input type="submit" value={"Rename"}/></td>
                    </tr>

            </table>
            </form>
            {new_path}
            </div>
        }
    }
}