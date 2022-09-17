use crate::models::{Node, NodeManager};

use super::{Event, Screen, ScreenFrame};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;
use std::sync::Arc;
use tokio::sync::Mutex;
use tui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct NodesListScreen {
    node_manager: Arc<Mutex<NodeManager>>,
    state: ListState,
    refresh_list: bool,
    cached_nodes: Vec<Node>,
}

impl NodesListScreen {
    pub fn new(node_manager: Arc<Mutex<NodeManager>>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self {
            node_manager,
            state,
            refresh_list: true,
            cached_nodes: vec![],
        }
    }
}

#[async_trait]
impl Screen for NodesListScreen {
    async fn paint(&mut self, frame: &mut ScreenFrame) {
        if self.refresh_list {
            self.cached_nodes = self.node_manager.clone().lock().await.list_nodes().await;
            self.refresh_list = false
        }
        let items = self
            .cached_nodes
            .iter()
            .map(|n| ListItem::new(n.name.clone()))
            .collect::<Vec<ListItem>>();
        let list = List::new(items)
            .block(Block::default().title("Nodes").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol(">>");
        let size = frame.size();

        frame.render_stateful_widget(list, size, &mut self.state);
    }

    async fn handle_input(&mut self, event: Event) -> Result<()> {
        if let Event::Input(event) = event {
            let selected = self.state.selected().unwrap_or(0);
            let selected = match event.code {
                KeyCode::Up => {
                    if selected == 0 {
                        self.cached_nodes.len() - 1
                    } else {
                        selected - 1
                    }
                }
                KeyCode::Down => {
                    if selected == self.cached_nodes.len() - 1 {
                        0
                    } else {
                        selected + 1
                    }
                }
                _ => 0,
            };
            self.state.select(Some(selected));
        }

        Ok(())
    }
}
