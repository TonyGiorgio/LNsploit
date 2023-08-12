use std::fmt::Display;

use tui::{
    backend::Backend,
    layout::Rect,
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use super::constants::{green, highlight, white, yellow};

pub fn draw_selectable_list<B, T>(
    f: &mut Frame<B>,
    // app: &App,
    layout_chunk: Rect,
    title: &str,
    items: &[T],
    highlight_state: (bool, bool),
    selected_index: Option<usize>,
) where
    B: Backend,
    T: Display,
{
    let mut state = ListState::default();
    state.select(selected_index);

    let lst_items: Vec<ListItem> = items
        .iter()
        .map(|i| ListItem::new(Span::raw(i.to_string())))
        .collect();

    let border_color_style = {
        if highlight_state.0 {
            yellow()
        } else if highlight_state.1 {
            green()
        } else {
            white()
        }
    };

    let list = List::new(lst_items)
        .block(
            Block::default()
                .title(Span::styled(title, white()))
                .borders(Borders::ALL)
                .border_style(border_color_style),
        )
        .style(white())
        .highlight_style(highlight())
        .highlight_symbol(">>");
    f.render_stateful_widget(list, layout_chunk, &mut state);
}
