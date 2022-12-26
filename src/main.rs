mod app;

use app::MainApp;

fn main() {
    yew::Renderer::<MainApp>::new().render();
}
