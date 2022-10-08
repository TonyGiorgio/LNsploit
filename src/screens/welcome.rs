use tui::{
    layout::Rect,
    style::Style,
    text::Text,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::widgets::constants::white;

use super::ScreenFrame;

const LOGO: &str = "
██╗     ███╗   ██╗███████╗██████╗ ██╗      ██████╗ ██╗████████╗
██║     ████╗  ██║██╔════╝██╔══██╗██║     ██╔═══██╗██║╚══██╔══╝
██║     ██╔██╗ ██║███████╗██████╔╝██║     ██║   ██║██║   ██║   
██║     ██║╚██╗██║╚════██║██╔═══╝ ██║     ██║   ██║██║   ██║   
███████╗██║ ╚████║███████║██║     ███████╗╚██████╔╝██║   ██║   
╚══════╝╚═╝  ╚═══╝╚══════╝╚═╝     ╚══════╝ ╚═════╝ ╚═╝   ╚═╝   
";

pub fn draw_welcome(frame: &mut ScreenFrame, chunk: Rect) {
    let text = Text::from(LOGO);
    let block = Paragraph::new(text)
        .style(white())
        .block(Block::default())
        .wrap(Wrap { trim: false })
        .block(Block::default().title("Home").borders(Borders::ALL));

    frame.render_widget(block, chunk);
}
