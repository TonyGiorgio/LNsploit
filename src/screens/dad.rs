use super::{draw_simulation, draw_welcome, AppEvent, Screen, ScreenFrame};
use crate::{
    application::AppState,
    handlers::{on_down_press_handler, on_up_press_handler},
    router::{Action, Location},
    widgets::draw::draw_selectable_list,
};
use anyhow::Result;
use async_trait::async_trait;
use bitcoin::psbt::raw::Key;
use crossterm::event::KeyCode;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, ListState},
};

const MAIN_MENU: [&str; 5] = [
    "Home",
    "Network View",
    "Routing",
    "Exploits",
    "Simulation Mode",
];

pub struct ParentScreen {
    pub menu_index: usize,
}

impl ParentScreen {
    pub fn new() -> Self {
        Self {
            menu_index: 0, // state,
                           // node_manager,
                           // nav_list: vec!["Nodes".into()],
                           // nav_list: MAIN_MENU,
        }
    }

    fn handle_enter(&self) -> Option<Action> {
        // let selected = self.state.selected().unwrap_or(0);
        // write!("handle enter");
        // dbg!("handle enter");
        let item = MAIN_MENU[self.menu_index];

        let action = match item {
            "Nodes" => Action::Push(Location::NodesList),
            "Simulation Mode" => Action::Push(Location::Simulation),
            _ => return None,
        };

        Some(action)
    }
}

#[async_trait]
impl Screen for ParentScreen {
    async fn paint(&self, frame: &mut ScreenFrame, state: &AppState) {
        let parent_chunks = Layout::default()
            .direction(Direction::Vertical)
            // TODO why won't Length work here?
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
            .split(frame.size());

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(parent_chunks[0]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(horizontal_chunks[0]);

        // Draw main menu
        draw_selectable_list(
            frame,
            chunks[0],
            "Menu",
            &MAIN_MENU,
            (false, false),
            Some(self.menu_index),
        );

        let nodes = state.node_manager.clone().lock().await.list_nodes().await;

        // Draw nodes list
        draw_selectable_list(
            frame,
            chunks[1],
            "Nodes",
            &nodes
                .iter()
                .map(|n| n.pubkey.clone())
                .collect::<Vec<String>>(),
            (false, false),
            None,
        );

        let nodes_block = Block::default().title("Nodes").borders(Borders::ALL);
        frame.render_widget(nodes_block, chunks[1]);

        // dbg!(state.router.get_current_route());
        // HERE'S WHERE THE MAGIC HAPPENS
        match state.router.get_current_route() {
            Location::Home => draw_welcome(frame, horizontal_chunks[1]),
            Location::Simulation => draw_simulation(frame, horizontal_chunks[1]),
            _ => draw_welcome(frame, horizontal_chunks[1]),
        };

        let footer_block = Block::default()
            .title("Keymap")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::LightBlue));
        frame.render_widget(footer_block, parent_chunks[1]);
    }

    async fn handle_input(
        &mut self,
        event: AppEvent,
        state: &mut AppState,
    ) -> Result<Option<Action>> {
        if let AppEvent::Input(event) = event {
            match event.code {
                KeyCode::Enter => return Ok(self.handle_enter()),
                KeyCode::Up => {
                    let next_index = on_up_press_handler(&MAIN_MENU, Some(self.menu_index));
                    self.menu_index = next_index;
                }
                KeyCode::Down => {
                    let next_index = on_down_press_handler(&MAIN_MENU, Some(self.menu_index));
                    self.menu_index = next_index;
                }
                _ => {}
            };
        }
        Ok(None)
    }
}
