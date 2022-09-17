use crate::screens::{Event, NodesListScreen, Screen};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::executor::block_on;
use std::io::{self, Stdout};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tui::{backend::CrosstermBackend, Terminal};

use crate::models::NodeManager;

pub struct Application {
    term: Terminal<CrosstermBackend<Stdout>>,
    node_manager: Arc<Mutex<NodeManager>>,
}

impl Application {
    pub async fn new() -> Result<Self> {
        let term = setup_terminal()?;
        let node_manager = NodeManager::new().await;

        Ok(Self {
            term,
            node_manager: Arc::new(Mutex::new(node_manager)),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut screen = NodesListScreen::new(self.node_manager.clone());
        let inputs = self.init_event_channel()?;

        loop {
            self.term.draw(|f| {
                let paint_future = screen.paint(f);
                block_on(paint_future);
            })?;

            match inputs.recv()? {
                Event::Quit => return Ok(()),
                event => screen.handle_input(event).await?,
            }
        }
    }

    fn init_event_channel(&self) -> Result<Receiver<Event>> {
        let (tx, rx) = mpsc::channel();
        let tick_rate = Duration::from_millis(400);

        tokio::spawn(async move {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                if event::poll(timeout).expect("poll works") {
                    if let CEvent::Key(key) = event::read().expect("can read events") {
                        let (app_event, exit) = match key.code {
                            KeyCode::Char('q') => (Event::Quit, true),
                            _ => (Event::Input(key), false),
                        };
                        tx.send(app_event).expect("can send events");
                        if exit {
                            return;
                        }
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if let Ok(_) = tx.send(Event::Tick) {
                        last_tick = Instant::now();
                    }
                }
            }
        });

        Ok(rx)
    }

    pub fn close(self) -> Result<()> {
        teardown_terminal(self.term)?;

        Ok(())
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

fn teardown_terminal(mut term: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;

    Ok(())
}
