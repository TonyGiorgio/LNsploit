use super::{AppEvent, Screen, ScreenFrame};
use crate::router::{Action, Location};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;
use tui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct HomeScreen {
    state: ListState,
    nav_list: Vec<String>,
}

impl HomeScreen {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self {
            state,
            nav_list: vec!["Nodes".into()],
        }
    }

    fn handle_enter(&self) -> Option<Action> {
        let selected = self.state.selected().unwrap_or(0);
        let item = self.nav_list[selected].as_str();

        let action = match item {
            "Nodes" => Action::Push(Location::NodesList),
            _ => return None,
        };

        Some(action)
    }
}

#[async_trait]
impl Screen for HomeScreen {
    async fn paint(&mut self, frame: &mut ScreenFrame) {
        let items = self
            .nav_list
            .iter()
            .map(|n| ListItem::new(n.clone()))
            .collect::<Vec<ListItem>>();

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
            let list_items = self.nav_list.len();

            match event.code {
                KeyCode::Enter => return Ok(self.handle_enter()),
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
