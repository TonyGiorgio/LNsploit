use super::Event;
use anyhow::Result;
use std::io::Stdout;
use tui::{backend::CrosstermBackend, Frame};

pub type ScreenFrame<'a> = Frame<'a, CrosstermBackend<Stdout>>;

pub trait Screen {
    fn paint(&mut self, frame: &mut ScreenFrame);
    fn handle_input(&mut self, event: Event) -> Result<()>;
}
