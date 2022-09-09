mod nodes_list;
mod screen;

pub use nodes_list::*;
pub use screen::*;

use crossterm::event::KeyEvent;

pub enum Event {
    Tick,
    Input(KeyEvent),
    Quit,
}
