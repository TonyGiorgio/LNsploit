use std::sync::Arc;

use super::{draw_simulation, draw_welcome, AppEvent, Screen, ScreenFrame, SIMULATION_MENU};
use crate::{
    application::AppState,
    handlers::{on_down_press_handler, on_up_press_handler},
    router::{Action, ActiveBlock, Location},
    widgets::draw::draw_selectable_list,
};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
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
    pub current_menu_list: Vec<String>,
}

impl ParentScreen {
    pub fn new() -> Self {
        Self {
            menu_index: 0,
            current_menu_list: MAIN_MENU
                .iter()
                .map(|x| String::from(*x))
                .collect::<Vec<String>>(),
        }
    }

    fn handle_enter(&mut self) -> Option<Action> {
        let item = self.current_menu_list[self.menu_index].clone();

        let (action, new_items) = match String::as_str(&item) {
            "Nodes" => (Action::Push(Location::NodesList), vec![]),
            "Simulation Mode" => (
                Action::Push(Location::Simulation),
                SIMULATION_MENU
                    .iter()
                    .map(|x| String::from(*x))
                    .collect::<Vec<String>>(),
            ),
            _ => return None,
        };

        self.current_menu_list = new_items;

        Some(action)
    }

    fn handle_esc(&mut self, state: &mut AppState) -> Option<Action> {
        // if the current active block is menu then do nothing
        match state.router.get_active_block() {
            ActiveBlock::Menu => return None,
            _ => (),
        };

        // reset menu list
        self.current_menu_list = MAIN_MENU
            .iter()
            .map(|x| String::from(*x))
            .collect::<Vec<String>>();

        // pop the current main screen
        // TODO consider leaving the current screen
        // but switching active block to menu instead
        Some(Action::Pop)
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

        let nav_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(horizontal_chunks[0]);

        // Draw main menu
        let home_active = {
            match state.router.get_active_block() {
                &ActiveBlock::Menu => (false, true),
                _ => (false, false),
            }
        };
        let home_selected_list = {
            if home_active.1 {
                Some(self.menu_index)
            } else {
                None
            }
        };

        draw_selectable_list(
            frame,
            nav_chunks[0],
            "Menu",
            &MAIN_MENU,
            home_active,
            home_selected_list,
        );

        // Draw nodes list
        // TODO how to switch to node list
        let node_active = {
            match state.router.get_active_block() {
                &ActiveBlock::Nodes => (false, true),
                _ => (false, false),
            }
        };
        draw_selectable_list(
            frame,
            nav_chunks[1],
            "Nodes",
            &state.cached_nodes_list,
            node_active,
            None,
        );

        let nodes_block = Block::default().title("Nodes").borders(Borders::ALL);
        frame.render_widget(nodes_block, nav_chunks[1]);

        // HERE'S WHERE THE MAGIC HAPPENS
        match state.router.get_current_route() {
            Location::Home => draw_welcome(frame, horizontal_chunks[1]),
            Location::Simulation => {
                let (is_active, menu_option) = {
                    let active_matches = matches!(
                        state.router.get_active_block(),
                        ActiveBlock::Main(Location::Simulation)
                    );
                    let menu_option = if active_matches {
                        Some(self.menu_index)
                    } else {
                        None
                    };
                    (active_matches, menu_option)
                };
                draw_simulation(frame, horizontal_chunks[1], (false, is_active), menu_option)
            }
            _ => draw_welcome(frame, horizontal_chunks[1]),
        };

        let footer_block = Paragraph::new(Text::from("q: Quit, N: Create new node")).block(
            Block::default()
                .title("Keymap")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::LightBlue)),
        );
        frame.render_widget(footer_block, parent_chunks[1]);
    }

    async fn handle_input(
        &mut self,
        event: AppEvent,
        state: &mut AppState,
    ) -> Result<Option<Action>> {
        if let AppEvent::Input(event) = event {
            match event.code {
                KeyCode::Char('N') => {
                    let _ = state.node_manager.clone().lock().await.new_node().await;

                    // Cache invalidation!
                    let nodes_list = {
                        let nodes = state.node_manager.clone().lock().await.list_nodes().await;
                        nodes
                            .iter()
                            .map(|n| n.pubkey.clone())
                            .collect::<Vec<String>>()
                    };
                    state.cached_nodes_list = Arc::new(nodes_list);
                }
                KeyCode::Esc => {
                    let new_action = self.handle_esc(state);
                    self.menu_index = 0; // reset when esc is pressed
                    return Ok(new_action);
                }
                KeyCode::Enter => {
                    let new_action = self.handle_enter();
                    self.menu_index = 0; // reset when enter is pressed
                    return Ok(new_action);
                }
                KeyCode::Up => {
                    let next_index =
                        on_up_press_handler(self.current_menu_list.clone(), Some(self.menu_index));
                    self.menu_index = next_index;
                }
                KeyCode::Down => {
                    let next_index = on_down_press_handler(
                        self.current_menu_list.clone(),
                        Some(self.menu_index),
                    );
                    self.menu_index = next_index;
                }
                _ => {}
            };
        }
        Ok(None)
    }
}
