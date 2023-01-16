use yew::prelude::*;


#[derive(Eq, PartialEq, Properties)]
struct ImageRenamerProps {
    original_path: String,
}

struct ImageRenamer {
    original_path: String,
    new_path: Option<String>,
}

enum ImageRenamerMsg {
    // Start,
    // Renaming,
    // Cancelled,
}

impl Component for ImageRenamer{
    type Message = ImageRenamerMsg;
    type Properties = ImageRenamerProps;

    fn create(ctx: &Context<Self>) -> Self {
        ImageRenamer {
            original_path: ctx.props().original_path.to_owned(),
            new_path: None,
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {

        let new_path = match self.new_path.clone() {
            Some(new_name) => html!{<p>{"to "}{new_name}</p>},
            None => html!{<></>}
        };

        html!{
            <>
            <p>{"Nothing here yet!"}</p>
            <p>{"Renaming "}{self.original_path.clone()}</p>
            {new_path}
            </>
        }
    }
}