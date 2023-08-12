use super::ScreenFrame;
use crate::{
    application::AppState,
    router::NodeSubLocation,
    widgets::{
        constants::{green, white, yellow},
        draw::draw_selectable_list,
    },
};
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
        NodeSubLocation::ListChannels => todo!(),
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
