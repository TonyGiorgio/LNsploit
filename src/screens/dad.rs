use std::sync::Arc;

use super::{
    draw_node, draw_simulation, draw_welcome, AppEvent, InputMode, Screen, ScreenFrame,
    NODE_ACTION_MENU, SIMULATION_MENU,
};
use crate::{
    application::AppState,
    handlers::{on_down_press_handler, on_up_press_handler},
    router::{Action, ActiveBlock, Location, NodeSubLocation},
    widgets::draw::draw_selectable_list,
};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;

use lightning::util::logger::{Logger, Record};
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

    fn handle_enter_main(&mut self) -> Option<Action> {
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

    fn handle_enter_node(&mut self) -> Option<Action> {
        let item = self.current_menu_list[self.menu_index].clone();

        let action = Action::Push(Location::Node(item, NodeSubLocation::ActionMenu));
        let new_items = NODE_ACTION_MENU
            .iter()
            .map(|x| String::from(*x))
            .collect::<Vec<String>>();

        self.current_menu_list = new_items;

        Some(action)
    }

    fn handle_enter_node_action(&self, pubkey: &str, state: &mut AppState) -> Option<Action> {
        let item = self.current_menu_list[self.menu_index].clone();

        let action = match String::as_str(&item) {
            "Connect Peer" => {
                // the next screen for connect peer will allow input
                state.input_mode = InputMode::Editing;
                Action::Push(Location::Node(pubkey.into(), NodeSubLocation::ConnectPeer))
            }
            _ => return None,
        };

        Some(action)
    }

    fn handle_esc(&mut self, state: &mut AppState) -> Option<Action> {
        // if the current active block and stack is menu then do nothing
        if matches!(state.router.get_active_block(), ActiveBlock::Menu)
            && matches!(state.router.get_current_route(), Location::Home)
        {
            return None;
        };
        // if the current active block is menu/nodes but active stack is something
        // else then replace back to the active stack
        if matches!(state.router.get_active_block(), ActiveBlock::Menu)
            || matches!(state.router.get_active_block(), ActiveBlock::Nodes)
        {
            if !matches!(state.router.get_current_route(), Location::Home)
                && !matches!(state.router.get_current_route(), Location::NodesList)
            {
                return Some(Action::Replace(state.router.get_current_route().clone()));
            }
        };

        // reset menu list
        // TODO: wish we had a slicker way of handling sub-menus
        self.current_menu_list = match state.router.get_current_route() {
            Location::Node(_, _) => NODE_ACTION_MENU
                .iter()
                .map(|x| String::from(*x))
                .collect::<Vec<String>>(),
            _ => MAIN_MENU
                .iter()
                .map(|x| String::from(*x))
                .collect::<Vec<String>>(),
        };

        self.menu_index = 0;

        // pop the current main screen
        Some(Action::Pop)
    }

    fn handle_enter_node_list(&mut self, state: &mut AppState) -> Option<Action> {
        // if the current active block is node list then do nothing
        match state.router.get_active_block() {
            ActiveBlock::Nodes => return None,
            _ => (),
        };

        // set menu list to node list
        self.current_menu_list = state
            .cached_nodes_list
            .iter()
            .map(|x| String::from(x))
            .collect::<Vec<String>>();

        Some(Action::Replace(Location::NodesList))
    }

    fn handle_enter_main_menu(&mut self, state: &mut AppState) -> Option<Action> {
        // if the current active block is node list then do nothing
        match state.router.get_active_block() {
            ActiveBlock::Menu => return None,
            _ => (),
        };

        // set menu list to menu items
        self.current_menu_list = MAIN_MENU
            .iter()
            .map(|x| String::from(*x))
            .collect::<Vec<String>>();

        Some(Action::Replace(Location::Home))
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
        let node_active = {
            match state.router.get_active_block() {
                &ActiveBlock::Nodes => (false, true),
                _ => (false, false),
            }
        };
        let node_selected_list = {
            if node_active.1 {
                Some(self.menu_index)
            } else {
                None
            }
        };
        draw_selectable_list(
            frame,
            nav_chunks[1],
            "Nodes",
            &state.cached_nodes_list,
            node_active,
            node_selected_list,
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
            Location::Node(n, s) => {
                let (is_active, menu_option, node_sub_location) = {
                    let active_matches = matches!(
                        state.router.get_active_block(),
                        ActiveBlock::Main(Location::Node(_, _))
                    );
                    let menu_option = if active_matches {
                        Some(self.menu_index)
                    } else {
                        None
                    };
                    (active_matches, menu_option, s)
                };
                draw_node(
                    frame,
                    horizontal_chunks[1],
                    n.clone(),
                    (false, is_active),
                    menu_option,
                    node_sub_location,
                )
            }
            _ => draw_welcome(frame, horizontal_chunks[1]),
        };

        let footer_block = Paragraph::new(Text::from(
            "q: Quit, esc: Back, M: Main Menu, L: Nodes Menu, N: Create new node",
        ))
        .block(
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
                KeyCode::Char('M') => {
                    let new_action = self.handle_enter_main_menu(state);
                    self.menu_index = 0; // reset when pressed
                    return Ok(new_action);
                }
                KeyCode::Char('L') => {
                    let new_action = self.handle_enter_node_list(state);
                    self.menu_index = 0; // reset when pressed
                    return Ok(new_action);
                }
                KeyCode::Esc => {
                    let new_action = self.handle_esc(state);
                    self.menu_index = 0; // reset when pressed
                    return Ok(new_action);
                }
                KeyCode::Enter => {
                    // check if enter is on main screen or node screen
                    let current_route = { state.router.get_active_block().clone() };
                    let new_action = match current_route {
                        ActiveBlock::Menu => self.handle_enter_main(),
                        ActiveBlock::Nodes => self.handle_enter_node(),
                        ActiveBlock::Main(location) => match location {
                            Location::Home => {
                                panic!("Shouldn't be possible");
                            }
                            Location::NodesList => {
                                panic!("Shouldn't be possible");
                            }
                            Location::Node(pubkey, sub_location) => {
                                let action = self.handle_enter_node_action(&pubkey, state);
                                state.logger.clone().log(&Record::new(
                                    lightning::util::logger::Level::Debug,
                                    format_args!(
                                        "action: {:?}, current sublocation: {:?}",
                                        action.clone(),
                                        sub_location.clone()
                                    ),
                                    "dad",
                                    "",
                                    334,
                                ));
                                action
                            }
                            Location::Simulation => {
                                panic!("Shouldn't be possible");
                            }
                        },
                        _ => None,
                    };
                    self.menu_index = 0; // reset when pressed
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
