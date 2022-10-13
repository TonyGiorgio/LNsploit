use tui::{
    layout::Rect,
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};

use crate::application::AppState;

use super::ScreenFrame;

pub fn draw_footer(frame: &mut ScreenFrame, chunk: Rect, state: &AppState) {
    if let Some(toast) = state.toast.clone() {
        let color = if toast.good_news {
            Color::Green
        } else {
            Color::Red
        };

        let toast_block = Paragraph::new(Text::from(toast.message.to_string())).block(
            Block::default()
                .title("Something Happened!")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color)),
        );
        frame.render_widget(toast_block, chunk);
    } else {
        let footer_block = Paragraph::new(Text::from(
            "q: Quit, esc: Back, M: Main Menu, L: Nodes Menu, N: Create new node",
        ))
        .block(
            Block::default()
                .title("Keymap")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::LightBlue)),
        );
        frame.render_widget(footer_block, chunk);
    }
}
