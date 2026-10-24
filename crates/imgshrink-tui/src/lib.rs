pub mod app;
pub mod terminal;
pub mod theme;
pub mod ui;

pub use app::App;

use imgshrink_core::config::Config;

pub fn run(config: Config, color: bool, ascii: bool, mouse: bool) -> std::io::Result<()> {
    terminal::install_panic_hook(mouse);
    let mut terminal = terminal::init(mouse)?;
    let mut app = App::new(config, color, ascii, mouse);
    let result = app.run(&mut terminal);
    terminal::restore(mouse);
    result
}
