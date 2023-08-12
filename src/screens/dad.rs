use super::{
    draw_exploits, draw_footer, draw_node, draw_simulation, draw_welcome, AppEvent, Screen,
    ScreenFrame, EXPLOIT_ACTION_MENU, NODE_ACTION_MENU, SIMULATION_MENU,
};
use crate::{
    application::AppState,
    handlers::{on_down_press_handler, on_up_press_handler},
    router::{Action, ActiveBlock, Location, NodeSubLocation},
    screens::{
        handle_connect_peer_action, handle_enter_exploit_action, handle_enter_main,
        handle_enter_main_menu, handle_enter_node, handle_enter_node_action,
        handle_enter_node_list, handle_force_close_prev_channel_action, handle_open_channel_action,
        handle_pay_invoice_action, ExploitAction, MenuAction, NodeAction, MAIN_MENU,
    },
    widgets::draw::draw_selectable_list,
};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;
use std::sync::Arc;
use tui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders},
};

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
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
        }
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
        if (matches!(state.router.get_active_block(), ActiveBlock::Menu)
            || matches!(state.router.get_active_block(), ActiveBlock::Nodes))
            && (!matches!(state.router.get_current_route(), Location::Home)
                && !matches!(state.router.get_current_route(), Location::NodesList))
        {
            return Some(Action::Replace(state.router.get_current_route().clone()));
        };

        // reset menu list
        self.set_list(state, Some(state.router.peak_next_stack().clone()));

        // pop the current main screen
        Some(Action::Pop)
    }

    fn set_list(&mut self, state: &mut AppState, next_location: Option<Location>) {
        let current_route = if let Some(next_location) = next_location {
            next_location
        } else {
            state.router.get_current_route().clone()
        };

        self.current_menu_list = match current_route {
            Location::Node(_, node_sub_location) => match node_sub_location {
                NodeSubLocation::ActionMenu => NODE_ACTION_MENU
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>(),
                NodeSubLocation::ConnectPeer => vec![], // NO LIST
                NodeSubLocation::PayInvoice => vec![],  // NO LIST
                NodeSubLocation::Suicide(channels) => channels,
                NodeSubLocation::ListChannels => vec![], // TODO
                NodeSubLocation::OpenChannel(pubkeys) => pubkeys,
                NodeSubLocation::NewAddress => vec![], // NO LIST
            },
            Location::NodesList => state
                .cached_nodes_list
                .iter()
                .map(String::from)
                .collect::<Vec<String>>(),
            Location::Exploits => EXPLOIT_ACTION_MENU
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
            Location::Home => MAIN_MENU
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
            Location::Simulation => SIMULATION_MENU
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
        };

        self.menu_index = 0;
    }
}

#[async_trait]
impl Screen for ParentScreen {
    async fn paint(&self, frame: &mut ScreenFrame, state: &AppState) {
        let parent_chunks = Layout::default()
            .direction(Direction::Vertical)
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
            Location::Exploits => {
                let (is_active, menu_option) = {
                    let active_matches = matches!(
                        state.router.get_active_block(),
                        ActiveBlock::Main(Location::Exploits)
                    );
                    let menu_option = if active_matches {
                        Some(self.menu_index)
                    } else {
                        None
                    };
                    (active_matches, menu_option)
                };
                draw_exploits(frame, horizontal_chunks[1], (false, is_active), menu_option)
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
                    state,
                )
            }
            _ => draw_welcome(frame, horizontal_chunks[1]),
        };

        draw_footer(frame, parent_chunks[1], state);
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
                    let (new_action, new_list) = handle_enter_main_menu(state);
                    if let Some(new_list) = new_list {
                        self.current_menu_list = new_list;
                    }
                    self.menu_index = 0; // reset when pressed
                    return Ok(new_action);
                }
                KeyCode::Char('L') => {
                    let (new_action, new_list) = handle_enter_node_list(state);
                    if let Some(new_list) = new_list {
                        self.current_menu_list = new_list;
                    }
                    self.menu_index = 0; // reset when pressed
                    return Ok(new_action);
                }
                KeyCode::Esc => {
                    let new_action = self.handle_esc(state);
                    // if esc does something, always try to reset items
                    let next_location = if let Some(new_action) = new_action.clone() {
                        match new_action {
                            Action::Push(location) => Some(location),
                            Action::Replace(location) => Some(location),
                            Action::Pop => Some(state.router.peak_next_stack().clone()),
                        }
                    } else {
                        Some(state.router.peak_next_stack().clone())
                    };
                    self.set_list(state, next_location);
                    return Ok(new_action);
                }
                KeyCode::Enter => {
                    // check the context for which the user is hitting enter on
                    let current_route = state.router.get_active_block().clone();
                    let current_item = self.current_menu_list[self.menu_index].clone();

                    // apply the enter onto the active screen
                    let (new_action, new_list) = match current_route {
                        ActiveBlock::Menu => {
                            let menu_action =
                                current_item.parse::<MenuAction>().unwrap_or_default();
                            handle_enter_main(state, menu_action)
                        }
                        ActiveBlock::Nodes => handle_enter_node(current_item),
                        ActiveBlock::Main(location) => match location {
                            Location::Home => (None, None),
                            Location::NodesList => (None, None),
                            Location::Exploits => {
                                let exploit_action =
                                    current_item.parse::<ExploitAction>().unwrap_or_default();
                                handle_enter_exploit_action(state, exploit_action).await
                            }
                            Location::Node(pubkey, sub_location) => match sub_location {
                                NodeSubLocation::ActionMenu => {
                                    let node_action =
                                        current_item.parse::<NodeAction>().unwrap_or_default();
                                    handle_enter_node_action(&pubkey, state, node_action).await
                                }
                                NodeSubLocation::ConnectPeer => {
                                    handle_connect_peer_action(&pubkey, state).await
                                }
                                NodeSubLocation::PayInvoice => {
                                    handle_pay_invoice_action(&pubkey, state).await
                                }
                                NodeSubLocation::OpenChannel(_) => {
                                    handle_open_channel_action(&pubkey, state, current_item).await
                                }
                                NodeSubLocation::Suicide(_) => {
                                    handle_force_close_prev_channel_action(
                                        &pubkey,
                                        state,
                                        current_item,
                                    )
                                    .await
                                }
                                NodeSubLocation::ListChannels => (None, None),
                                NodeSubLocation::NewAddress => (None, None),
                            },
                            Location::Simulation => (None, None),
                        },
                    };

                    // if enter does something, always try to reset items
                    if let Some(new_list) = new_list {
                        self.current_menu_list = new_list;
                    }
                    if let Some(new_action) = new_action.clone() {
                        self.set_list(
                            state,
                            match new_action {
                                Action::Push(location) => Some(location),
                                Action::Replace(location) => Some(location),
                                Action::Pop => Some(state.router.peak_next_stack().clone()),
                            },
                        );
                    }

                    self.menu_index = 0; // always reset when pressed
                    return Ok(new_action);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let next_index =
                        on_up_press_handler(self.current_menu_list.clone(), Some(self.menu_index));
                    self.menu_index = next_index;
                }
                KeyCode::Down | KeyCode::Char('j') => {
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
