mod dad;
mod exploit;
mod node;
mod screen;
mod simulation;
mod welcome;

pub use dad::*;
pub use exploit::*;
pub use node::*;
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
