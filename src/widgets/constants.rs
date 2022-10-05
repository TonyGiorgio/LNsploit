use tui::style::{Color, Modifier, Style};

pub fn white() -> Style {
    Style::default().fg(Color::White)
}

pub fn highlight() -> Style {
    Style::default()
        .add_modifier(Modifier::ITALIC)
        .fg(Color::Black)
        .bg(Color::Gray)
}
