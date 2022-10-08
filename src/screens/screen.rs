use super::AppEvent;
use crate::{
    application::{AppState, Application},
    router::{Action, Location},
};
use anyhow::Result;
use async_trait::async_trait;
use std::io::Stdout;
use tui::{backend::CrosstermBackend, Frame};

pub type ScreenFrame<'a> = Frame<'a, CrosstermBackend<Stdout>>;

#[async_trait]
pub trait Screen {
    async fn paint(&self, frame: &mut ScreenFrame, state: &AppState);
    async fn handle_input(
        &mut self,
        event: AppEvent,
        state: &mut AppState,
    ) -> Result<Option<Action>>;
}
