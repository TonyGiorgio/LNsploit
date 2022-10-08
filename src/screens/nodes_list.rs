use super::{AppEvent, Screen, ScreenFrame};
use crate::models::{Node, NodeManager};
use crate::router::{Action, Location};
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

    fn handle_enter_node(&self, pubkey: String, node_id: String) -> Option<Action> {
        Some(Action::Push(Location::Node(pubkey, node_id)))
    }
}

#[async_trait]
impl Screen for NodesListScreen {
    async fn paint(&mut self, frame: &mut ScreenFrame) {
        if self.refresh_list {
            self.cached_nodes = self.node_manager.clone().lock().await.list_nodes().await;
            self.refresh_list = false
        }

        // The first item in the list is a "[New Node]" action
        // Kind of a hack though
        let mut items = vec![ListItem::new("[New Node]")];
        let node_items = self
            .cached_nodes
            .iter()
            .map(|n| ListItem::new(n.pubkey.clone()))
            .collect::<Vec<ListItem>>();
        items.append(&mut node_items.clone());

        let list = List::new(items)
            .block(Block::default().title("Nodes").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol(">>");
        let size = frame.size();

        frame.render_stateful_widget(list, size, &mut self.state);
    }

    async fn handle_input(&mut self, event: AppEvent) -> Result<Option<Action>> {
        if let AppEvent::Input(event) = event {
            let selected = self.state.selected().unwrap_or(0);
            let list_items = self.cached_nodes.len() + 1; // + 1 for the [New Node] action

            match event.code {
                KeyCode::Enter => {
                    if selected == 0 {
                        // This is the [New Node] action, go to the new node screen
                        let _ = self.node_manager.clone().lock().await.new_node().await;
                        // TODO consider an error screen if an error exists
                        self.refresh_list = true;
                    } else {
                        // selected a certain node, go to the node screen
                        return Ok(self.handle_enter_node(
                            self.cached_nodes[selected - 1].pubkey.clone(),
                            self.cached_nodes[selected - 1].id.clone(),
                        ));
                    }
                }
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
