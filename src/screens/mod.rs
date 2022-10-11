mod dad;
mod node;
mod nodes_list;
mod screen;
mod simulation;
mod welcome;

pub use dad::*;
pub use node::*;
pub use nodes_list::*;
pub use screen::*;
pub use simulation::*;
pub use welcome::*;

use crossterm::event::KeyEvent;

#[derive(Debug)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    Input(KeyEvent),
    Paste,
    Copy,
    Back,
    Quit,
}
