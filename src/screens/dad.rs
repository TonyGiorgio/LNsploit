use std::sync::Arc;

use super::{
    draw_exploits, draw_node, draw_simulation, draw_welcome, AppEvent, InputMode, Screen,
    ScreenFrame, EXPLOIT_ACTION_MENU, NODE_ACTION_MENU, SIMULATION_MENU,
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

    fn handle_enter_main(&mut self, state: &mut AppState) -> Option<Action> {
        let item = self.current_menu_list[self.menu_index].clone();

        let action = match String::as_str(&item) {
            "Nodes" => Action::Push(Location::NodesList),
            "Simulation Mode" => Action::Push(Location::Simulation),
            "Exploits" => Action::Push(Location::Exploits),
            _ => return None,
        };

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

    async fn handle_enter_node_action(&self, pubkey: &str, state: &mut AppState) -> Option<Action> {
        let item = self.current_menu_list[self.menu_index].clone();

        let action = match String::as_str(&item) {
            "Connect Peer" => {
                // the next screen for connect peer will allow input
                // TODO i don't think this is ever used
                state.input_mode = InputMode::Editing;
                Action::Push(Location::Node(pubkey.into(), NodeSubLocation::ConnectPeer))
            }
            "Pay" => {
                // the next screen for pay invoice will allow input
                // TODO i don't think this is ever used
                state.input_mode = InputMode::Editing;
                Action::Push(Location::Node(pubkey.into(), NodeSubLocation::PayInvoice))
            }
            "Open Channel" => {
                // get the list of nodes that the peer is connect to to open channel with
                let node_id = state
                    .node_manager
                    .lock()
                    .await
                    .get_node_id_by_pubkey(pubkey)
                    .await
                    .expect("Pubkey should have corresponding node_id");

                let peer_pubkeys = state.node_manager.lock().await.list_peers(node_id);

                Action::Push(Location::Node(
                    pubkey.into(),
                    NodeSubLocation::OpenChannel(peer_pubkeys),
                ))
            }
            _ => return None,
        };

        Some(action)
    }

    async fn handle_connect_peer_action(
        &self,
        pubkey: &str,
        state: &mut AppState,
    ) -> Option<Action> {
        let node_id = state
            .node_manager
            .lock()
            .await
            .get_node_id_by_pubkey(pubkey)
            .await
            .expect("Pubkey should have corresponding node_id");

        if let Some(peer_connection_string) = state.paste_contents.clone() {
            match state
                .node_manager
                .lock()
                .await
                .connect_peer(node_id, peer_connection_string.to_string())
                .await
            {
                Ok(_) => {
                    state.logger.clone().log(&Record::new(
                        lightning::util::logger::Level::Info,
                        format_args!("connected to peer"),
                        "dad",
                        "",
                        334,
                    ));
                }
                Err(e) => {
                    state.logger.clone().log(&Record::new(
                        lightning::util::logger::Level::Error,
                        format_args!("{}", e),
                        "dad",
                        "",
                        334,
                    ));
                }
            }
        }

        None
    }

    async fn handle_pay_invoice_action(
        &self,
        pubkey: &str,
        state: &mut AppState,
    ) -> Option<Action> {
        let node_id = state
            .node_manager
            .lock()
            .await
            .get_node_id_by_pubkey(pubkey)
            .await
            .expect("Pubkey should have corresponding node_id");

        if let Some(invoice_string) = state.paste_contents.clone() {
            match state
                .node_manager
                .lock()
                .await
                .pay_invoice(node_id, invoice_string.to_string())
            {
                Ok(_) => {
                    state.logger.clone().log(&Record::new(
                        lightning::util::logger::Level::Info,
                        format_args!("initiated invoice payment"),
                        "dad",
                        "",
                        334,
                    ));
                }
                Err(e) => {
                    state.logger.clone().log(&Record::new(
                        lightning::util::logger::Level::Error,
                        format_args!("{}", e),
                        "dad",
                        "",
                        334,
                    ));
                }
            }
        }

        None
    }

    async fn handle_open_channel_action(
        &self,
        pubkey: &str,
        state: &mut AppState,
    ) -> Option<Action> {
        let item = self.current_menu_list[self.menu_index].clone();

        let node_id = state
            .node_manager
            .lock()
            .await
            .get_node_id_by_pubkey(pubkey)
            .await
            .expect("Pubkey should have corresponding node_id");

        match state
            .node_manager
            .lock()
            .await
            .open_channel(node_id, item, 100_000)
            .await
        {
            Ok(_) => {
                state.logger.clone().log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("Opened channel to peer"),
                    "dad",
                    "",
                    334,
                ));
                None
            }
            Err(e) => {
                state.logger.clone().log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("{}", e),
                    "dad",
                    "",
                    334,
                ));
                None
            }
        }
    }

    async fn handle_enter_exploit_action(&self, state: &mut AppState) -> Option<Action> {
        let action = match self.menu_index {
            0 => {
                // Broadcast LND tx
                match state.node_manager.lock().await.broadcast_lnd_15_exploit() {
                    Ok(_) => {
                        state.logger.clone().log(&Record::new(
                            lightning::util::logger::Level::Debug,
                            format_args!("broadcasted tx!"),
                            "dad",
                            "",
                            334,
                        ));
                    }
                    Err(e) => {
                        state.logger.clone().log(&Record::new(
                            lightning::util::logger::Level::Debug,
                            format_args!("failure to broadcast tx: {}", e),
                            "dad",
                            "",
                            334,
                        ));
                    }
                }
                None
            }
            _ => return None,
        };

        action
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
                    .map(|x| String::from(*x))
                    .collect::<Vec<String>>(),
                NodeSubLocation::ConnectPeer => vec![], // NO LIST
                NodeSubLocation::PayInvoice => vec![],  // NO LIST
                NodeSubLocation::ListChannels => vec![], // TODO
                NodeSubLocation::OpenChannel(pubkeys) => pubkeys.clone(),
                NodeSubLocation::NewAddress => vec![], // NO LIST
            },
            Location::NodesList => state
                .cached_nodes_list
                .iter()
                .map(|x| String::from(x))
                .collect::<Vec<String>>(),
            Location::Exploits => EXPLOIT_ACTION_MENU
                .iter()
                .map(|x| String::from(*x))
                .collect::<Vec<String>>(),
            Location::Home => MAIN_MENU
                .iter()
                .map(|x| String::from(*x))
                .collect::<Vec<String>>(),
            Location::Simulation => SIMULATION_MENU
                .iter()
                .map(|x| String::from(*x))
                .collect::<Vec<String>>(),
        };

        self.menu_index = 0;
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
                    // if esc does something, always try to reset items
                    let next_location = if let Some(new_action) = new_action.clone() {
                        match new_action.clone() {
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
                    // check if enter is on main screen or node screen
                    let current_route = { state.router.get_active_block().clone() };
                    let new_action = match current_route {
                        ActiveBlock::Menu => self.handle_enter_main(state),
                        ActiveBlock::Nodes => self.handle_enter_node(),
                        ActiveBlock::Main(location) => match location {
                            Location::Home => None,
                            Location::NodesList => None,
                            Location::Exploits => self.handle_enter_exploit_action(state).await,
                            Location::Node(pubkey, sub_location) => match sub_location {
                                NodeSubLocation::ActionMenu => {
                                    let action =
                                        self.handle_enter_node_action(&pubkey, state).await;
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
                                NodeSubLocation::ConnectPeer => {
                                    let action =
                                        self.handle_connect_peer_action(&pubkey, state).await;
                                    action
                                }
                                NodeSubLocation::PayInvoice => {
                                    let action =
                                        self.handle_pay_invoice_action(&pubkey, state).await;
                                    action
                                }
                                NodeSubLocation::OpenChannel(_) => {
                                    let action =
                                        self.handle_open_channel_action(&pubkey, state).await;
                                    action
                                }
                                NodeSubLocation::ListChannels => None,
                                NodeSubLocation::NewAddress => None,
                            },
                            Location::Simulation => None,
                        },
                        _ => None,
                    };

                    // if enter does something, always try to reset items
                    if let Some(new_action) = new_action.clone() {
                        self.set_list(
                            state,
                            match new_action.clone() {
                                Action::Push(location) => Some(location),
                                Action::Replace(location) => Some(location),
                                Action::Pop => Some(state.router.peak_next_stack().clone()),
                            },
                        );
                    }

                    self.menu_index = 0; // always reset when pressed
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
