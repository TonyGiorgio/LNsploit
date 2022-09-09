use super::{Event, Screen, ScreenFrame};
use crate::models::NodeList;
use anyhow::Result;
use crossterm::event::KeyCode;
use tui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct NodesListScreen {
    nodes: NodeList,
    state: ListState,
}

impl NodesListScreen {
    pub fn new(nodes: NodeList) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self { nodes, state }
    }
}

impl Screen for NodesListScreen {
    fn paint(&mut self, frame: &mut ScreenFrame) {
        let items = self
            .nodes
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

    fn handle_input(&mut self, event: Event) -> Result<()> {
        if let Event::Input(event) = event {
            let selected = self.state.selected().unwrap_or(0);
            let selected = match event.code {
                KeyCode::Up => {
                    if selected == 0 {
                        self.nodes.len() - 1
                    } else {
                        selected - 1
                    }
                }
                KeyCode::Down => {
                    if selected == self.nodes.len() - 1 {
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
