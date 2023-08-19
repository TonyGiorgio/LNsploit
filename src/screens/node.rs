use super::ScreenFrame;
use crate::{
    application::{AppState, Toast},
    models::hex_str,
    router::{Action, Location, NodeSubLocation},
    screens::{InputMode, MenuItemData},
    widgets::{
        constants::{green, white, yellow},
        draw::draw_selectable_list,
    },
};
use lightning::util::logger::{Logger, Record};
use std::sync::Arc;
use std::{fmt, str::FromStr};
use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

#[derive(Default)]
pub enum NodeAction {
    ConnectPeer,
    OpenChannel,
    ListChannels,
    Invoice,
    Pay,
    NewOnChainAddress,
    BroadcastRevokedCommitmentTransaction,
    #[default]
    Invalid,
}

impl fmt::Display for NodeAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NodeAction::ConnectPeer => write!(f, "Connect Peer"),
            NodeAction::OpenChannel => write!(f, "Open Channel"),
            NodeAction::ListChannels => write!(f, "List Channels"),
            NodeAction::Invoice => write!(f, "Invoice"),
            NodeAction::Pay => write!(f, "Pay"),
            NodeAction::NewOnChainAddress => write!(f, "New On Chain Address"),
            NodeAction::BroadcastRevokedCommitmentTransaction => {
                write!(f, "Broadcast revoked commitment transaction")
            }
            NodeAction::Invalid => write!(f, "Invalid"),
        }
    }
}

impl FromStr for NodeAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Connect Peer" => Ok(NodeAction::ConnectPeer),
            "Open Channel" => Ok(NodeAction::OpenChannel),
            "List Channels" => Ok(NodeAction::ListChannels),
            "Invoice" => Ok(NodeAction::Invoice),
            "Pay" => Ok(NodeAction::Pay),
            "New On Chain Address" => Ok(NodeAction::NewOnChainAddress),
            "Broadcast revoked commitment transaction" => {
                Ok(NodeAction::BroadcastRevokedCommitmentTransaction)
            }
            _ => Ok(NodeAction::Invalid),
        }
    }
}

pub const NODE_ACTION_MENU: [NodeAction; 7] = [
    NodeAction::ConnectPeer,
    NodeAction::OpenChannel,
    NodeAction::ListChannels,
    NodeAction::Invoice,
    NodeAction::Pay,
    NodeAction::NewOnChainAddress,
    NodeAction::BroadcastRevokedCommitmentTransaction,
];

// enters into the specific node pubkey from the node list
pub fn handle_enter_node(
    pubkey: MenuItemData,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
    let action_item = match pubkey {
        MenuItemData::NodePubkey(s) => s,
        _ => panic!("should be pubkey"),
    };
    let action = Action::Push(Location::Node(action_item, NodeSubLocation::ActionMenu));
    let new_items = NODE_ACTION_MENU
        .iter()
        .map(|x| (x.to_string(), MenuItemData::GenericString(x.to_string())))
        .collect::<Vec<_>>();

    (Some(action), Some(new_items))
}

pub async fn handle_enter_node_action(
    pubkey: &str,
    state: &mut AppState,
    node_action: NodeAction,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
    let action = match node_action {
        NodeAction::ConnectPeer => {
            // the next screen for connect peer will allow input
            // TODO i don't think this is ever used
            state.input_mode = InputMode::Editing;
            Action::Push(Location::Node(pubkey.into(), NodeSubLocation::ConnectPeer))
        }
        NodeAction::Pay => {
            // the next screen for pay invoice will allow input
            // TODO i don't think this is ever used
            state.input_mode = InputMode::Editing;
            Action::Push(Location::Node(pubkey.into(), NodeSubLocation::PayInvoice))
        }
        NodeAction::BroadcastRevokedCommitmentTransaction => {
            // no next screen, just a force close action
            let node_id = state
                .node_manager
                .lock()
                .await
                .get_node_id_by_pubkey(pubkey)
                .await
                .expect("Pubkey should have corresponding node_id");

            let channels = state
                .node_manager
                .lock()
                .await
                .list_channels(node_id)
                .iter()
                .map(|c| {
                    c.counterparty.node_id.to_string()
                        + ":"
                        + String::as_str(&hex_str(&c.channel_id))
                })
                .collect();

            state.logger.clone().log(&Record::new(
                lightning::util::logger::Level::Debug,
                format_args!("channels: {:?}", channels),
                "dad",
                "",
                334,
            ));

            Action::Push(Location::Node(
                pubkey.into(),
                NodeSubLocation::Suicide(channels),
            ))
        }
        NodeAction::OpenChannel => {
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
        NodeAction::ListChannels => {
            // no next screen, just a force close action
            let node_id = state
                .node_manager
                .lock()
                .await
                .get_node_id_by_pubkey(pubkey)
                .await
                .expect("Pubkey should have corresponding node_id");

            let channels = state
                .node_manager
                .lock()
                .await
                .list_channels(node_id)
                .iter()
                .map(|c| {
                    c.counterparty.node_id.to_string()
                        + ":"
                        + String::as_str(&hex_str(&c.channel_id))
                })
                .collect();

            state.logger.clone().log(&Record::new(
                lightning::util::logger::Level::Debug,
                format_args!("channels: {:?}", channels),
                "dad",
                "",
                334,
            ));

            Action::Push(Location::Node(
                pubkey.into(),
                NodeSubLocation::ListChannels(channels),
            ))
        }
        NodeAction::Invoice => return (None, None),
        NodeAction::NewOnChainAddress => return (None, None),
        NodeAction::Invalid => return (None, None),
    };

    (Some(action), None)
}

pub async fn handle_connect_peer_action(
    pubkey: &str,
    state: &mut AppState,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
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
                state.toast = Some(Toast::new("Connected to peer", true));
                state.logger.clone().log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("connected to peer"),
                    "dad",
                    "",
                    334,
                ));
            }
            Err(e) => {
                state.toast = Some(Toast::new("Could not connect to peer", false));
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

    (None, None)
}

pub async fn handle_pay_invoice_action(
    pubkey: &str,
    state: &mut AppState,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
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
                state.toast = Some(Toast::new("Initiated payment", true));
                state.logger.clone().log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("initiated invoice payment"),
                    "dad",
                    "",
                    334,
                ));
            }
            Err(e) => {
                state.toast = Some(Toast::new("Failed to initiated payment", false));
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

    (None, None)
}

pub async fn handle_open_channel_action(
    pubkey: &str,
    state: &mut AppState,
    item_action: String,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
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
        .open_channel(node_id, item_action, 100_000)
        .await
    {
        Ok(_) => {
            state.toast = Some(Toast::new("Opened channel to peer", true));
            state.logger.clone().log(&Record::new(
                lightning::util::logger::Level::Info,
                format_args!("Opened channel to peer"),
                "dad",
                "",
                334,
            ));
            (None, None)
        }
        Err(e) => {
            state.toast = Some(Toast::new("Failed to open channel to peer", false));
            state.logger.clone().log(&Record::new(
                lightning::util::logger::Level::Error,
                format_args!("{}", e),
                "dad",
                "",
                334,
            ));
            (None, None)
        }
    }
}

pub async fn handle_force_close_prev_channel_action(
    pubkey: &str,
    state: &mut AppState,
    item_action: String,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
    let mut items = item_action.split(':');
    let counterparty_pubkey = items.next();
    let channel_id = items.next();

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
        .force_close_channel_with_initial_state(
            node_id,
            String::from(channel_id.unwrap()),
            String::from(counterparty_pubkey.unwrap()),
        )
        .await
    {
        Ok(_) => {
            state.toast = Some(Toast::new("Force closed channel", true));
            state.logger.clone().log(&Record::new(
                lightning::util::logger::Level::Info,
                format_args!("force closed transaction"),
                "dad",
                "",
                334,
            ));
            (None, None)
        }
        Err(e) => {
            state.toast = Some(Toast::new("Failed to force close channel", false));
            state.logger.clone().log(&Record::new(
                lightning::util::logger::Level::Error,
                format_args!("{}", e),
                "dad",
                "",
                334,
            ));
            (None, None)
        }
    }
}

pub fn draw_node(
    frame: &mut ScreenFrame,
    chunk: Rect,
    pubkey: String,
    highlight_state: (bool, bool),
    menu_index: Option<usize>,
    sub_location: &NodeSubLocation,
    state: &AppState,
) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunk);
    let text = Text::from(format!("Node Pubkey: {}", pubkey));

    let block = Paragraph::new(text)
        .style(white())
        .block(Block::default())
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title("Node View")
                .borders(Borders::ALL)
                .border_style(white()),
        );

    frame.render_widget(block, vertical_chunks[0]);
    match sub_location {
        NodeSubLocation::ActionMenu => {
            draw_selectable_list(
                frame,
                vertical_chunks[1],
                "Node Actions",
                &NODE_ACTION_MENU,
                highlight_state,
                menu_index,
            );
        }
        NodeSubLocation::ConnectPeer => {
            draw_connect_peer(frame, vertical_chunks[1], highlight_state, state)
        }
        NodeSubLocation::PayInvoice => {
            draw_pay_invoice(frame, vertical_chunks[1], highlight_state, state)
        }
        NodeSubLocation::Suicide(channels) => draw_force_close_channel(
            frame,
            vertical_chunks[1],
            highlight_state,
            state,
            menu_index,
            channels.clone(),
        ),
        NodeSubLocation::ListChannels(channel_ids) => draw_list_channels(
            frame,
            vertical_chunks[1],
            highlight_state,
            state,
            menu_index,
            channel_ids.clone(),
        ),
        NodeSubLocation::OpenChannel(node_pubkeys) => draw_open_channel(
            frame,
            vertical_chunks[1],
            highlight_state,
            state,
            menu_index,
            node_pubkeys.clone(),
        ),
        NodeSubLocation::NewAddress => todo!(),
    }
}

fn draw_connect_peer(
    frame: &mut ScreenFrame,
    chunk: Rect,
    highlight_state: (bool, bool),
    state: &AppState,
) {
    let border_color_style = {
        if highlight_state.0 {
            yellow()
        } else if highlight_state.1 {
            green()
        } else {
            white()
        }
    };

    let paste = if let Some(paste) = state.paste_contents.clone() {
        paste
    } else {
        Arc::new("".into())
    };

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(chunk);

    let outline = Block::default()
        .title("Connect Peer")
        .borders(Borders::ALL)
        .border_style(border_color_style);

    frame.render_widget(outline, chunk);

    let text = Text::from("p2p connection string: (ctrl+v to paste it in)");
    let help_text = Paragraph::new(text).style(white()).block(Block::default());

    frame.render_widget(help_text, inner_chunks[0]);

    let textbox = Paragraph::new(Text::from(paste.to_string()))
        .style(white())
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(textbox, inner_chunks[1]);
}

fn draw_pay_invoice(
    frame: &mut ScreenFrame,
    chunk: Rect,
    highlight_state: (bool, bool),
    state: &AppState,
) {
    let border_color_style = {
        if highlight_state.0 {
            yellow()
        } else if highlight_state.1 {
            green()
        } else {
            white()
        }
    };

    let paste = if let Some(paste) = state.paste_contents.clone() {
        paste
    } else {
        Arc::new("".into())
    };

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(chunk);

    let outline = Block::default()
        .title("Pay Invoice")
        .borders(Borders::ALL)
        .border_style(border_color_style);

    frame.render_widget(outline, chunk);

    let text = Text::from("bolt11 invoice: (ctrl+v to paste it in)");
    let help_text = Paragraph::new(text).style(white()).block(Block::default());

    frame.render_widget(help_text, inner_chunks[0]);

    let textbox = Paragraph::new(Text::from(paste.to_string()))
        .style(white())
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(textbox, inner_chunks[1]);
}

fn draw_force_close_channel(
    frame: &mut ScreenFrame,
    chunk: Rect,
    highlight_state: (bool, bool),
    _state: &AppState,
    menu_index: Option<usize>,
    channels: Vec<String>,
) {
    let _border_color_style = {
        if highlight_state.0 {
            yellow()
        } else if highlight_state.1 {
            green()
        } else {
            white()
        }
    };

    draw_selectable_list(
        frame,
        chunk,
        "Select a channel to force close",
        &channels,
        highlight_state,
        menu_index,
    )
}

fn draw_open_channel(
    frame: &mut ScreenFrame,
    chunk: Rect,
    highlight_state: (bool, bool),
    _state: &AppState,
    menu_index: Option<usize>,
    node_pubkeys: Vec<String>,
) {
    let _border_color_style = {
        if highlight_state.0 {
            yellow()
        } else if highlight_state.1 {
            green()
        } else {
            white()
        }
    };

    draw_selectable_list(
        frame,
        chunk,
        "Select a node",
        &node_pubkeys,
        highlight_state,
        menu_index,
    )
}

fn draw_list_channels(
    frame: &mut ScreenFrame,
    chunk: Rect,
    highlight_state: (bool, bool),
    _state: &AppState,
    menu_index: Option<usize>,
    channels: Vec<String>,
) {
    let _border_color_style = {
        if highlight_state.0 {
            yellow()
        } else if highlight_state.1 {
            green()
        } else {
            white()
        }
    };

    draw_selectable_list(
        frame,
        chunk,
        "Select a channel to view",
        &channels,
        highlight_state,
        menu_index,
    )
}
