use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::widgets::{constants::white, draw::draw_selectable_list};

use super::ScreenFrame;

const LOGO: &str = "
Hey how is everyone doing? Let's do some SIMULATION
";

pub const SIMULATION_MENU: [&str; 3] = ["Hello", "Welcome", "Goodbye"];

pub fn draw_simulation(
    frame: &mut ScreenFrame,
    chunk: Rect,
    highlight_state: (bool, bool),
    menu_index: Option<usize>,
) {
    let horizontal_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunk);

    let text = Text::from(LOGO);

    let block = Paragraph::new(text)
        .style(white())
        .block(Block::default())
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title("Simulation")
                .borders(Borders::ALL)
                .border_style(white()),
        );

    frame.render_widget(block, horizontal_chunks[0]);

    draw_selectable_list(
        frame,
        horizontal_chunks[1],
        "Simulation Actions",
        &SIMULATION_MENU,
        highlight_state,
        menu_index,
    )
}
