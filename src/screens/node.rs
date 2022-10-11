use std::sync::Arc;

use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    application::AppState,
    router::NodeSubLocation,
    widgets::{
        constants::{green, white, yellow},
        draw::draw_selectable_list,
    },
};

use super::ScreenFrame;

pub const NODE_ACTION_MENU: [&str; 5] = [
    "Connect Peer",
    "List Channels",
    "Invoice",
    "Pay",
    "New On Chain Address",
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
    let text = Text::from(format!("Node Pubkey: {}", pubkey.clone()));

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
        NodeSubLocation::ListChannels => todo!(),
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

    let text = Text::from(format!("p2p connection string: (paste it in)"));
    let help_text = Paragraph::new(text).style(white()).block(Block::default());

    frame.render_widget(help_text, inner_chunks[0]);

    let textbox = Paragraph::new(Text::from(paste.to_string()))
        .style(white())
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(textbox, inner_chunks[1]);
}
