mod home;
mod nodes_list;
mod screen;

pub use home::*;
pub use nodes_list::*;
pub use screen::*;

use crossterm::event::KeyEvent;

pub enum AppEvent {
    Tick,
    Input(KeyEvent),
    Back,
    Quit,
}
