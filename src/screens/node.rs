use super::{AppEvent, Screen, ScreenFrame};
use crate::models::{Node, NodeManager};
use crate::router::Action;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;
use std::sync::Arc;
use tokio::sync::Mutex;
use tui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct NodeScreen {
    node_manager: Arc<Mutex<NodeManager>>,
    state: ListState,
    pubkey: String,
}

impl NodeScreen {
    pub fn new(node_manager: Arc<Mutex<NodeManager>>, pubkey: String) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self {
            node_manager,
            state,
            pubkey,
        }
    }
}

#[async_trait]
impl Screen for NodeScreen {
    async fn paint(&mut self, frame: &mut ScreenFrame) {
        let items = vec![ListItem::new("[Back]")];
        let list = List::new(items)
            .block(
                Block::default()
                    .title(self.pubkey.clone())
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol(">>");
        let size = frame.size();

        frame.render_stateful_widget(list, size, &mut self.state);
    }

    async fn handle_input(&mut self, event: AppEvent) -> Result<Option<Action>> {
        if let AppEvent::Input(event) = event {
            let selected = self.state.selected().unwrap_or(0);
            let list_items = 1;

            match event.code {
                KeyCode::Enter => {}
                KeyCode::Up => {
                    if selected == 0 {
                        self.state.select(Some(list_items - 1));
                    } else {
                        self.state.select(Some(selected - 1));
                    }
                }
                KeyCode::Down => {
                    if selected == list_items - 1 {
                        self.state.select(Some(0));
                    } else {
                        self.state.select(Some(selected + 1));
                    }
                }
                _ => (),
            };
        }

        Ok(None)
    }
}
