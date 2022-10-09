use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::widgets::{constants::white, draw::draw_selectable_list};

use super::ScreenFrame;

pub const NODE_MENU: [&str; 5] = [
    "Connect",
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
) {
    let horizontal_chunks = Layout::default()
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

    frame.render_widget(block, horizontal_chunks[0]);

    draw_selectable_list(
        frame,
        horizontal_chunks[1],
        "Node Actions",
        &NODE_MENU,
        highlight_state,
        menu_index,
    )
}
