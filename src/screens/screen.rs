use super::Event;
use anyhow::Result;
use async_trait::async_trait;
use std::io::Stdout;
use tui::{backend::CrosstermBackend, Frame};

pub type ScreenFrame<'a> = Frame<'a, CrosstermBackend<Stdout>>;

#[async_trait]
pub trait Screen {
    async fn paint(&mut self, frame: &mut ScreenFrame);
    async fn handle_input(&mut self, event: Event) -> Result<()>;
}
