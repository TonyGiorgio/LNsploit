use crate::models::NodeManager;
use crate::router::{Action, Router};
use crate::screens::{AppEvent, HomeScreen, Screen};
use anyhow::{anyhow, Result};
use bitcoincore_rpc::Client;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::SqliteConnection;
use futures::executor::block_on;
use std::io::{self, Stdout};
use std::sync::{
    mpsc::{self, Receiver},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tui::{backend::CrosstermBackend, Terminal};

pub struct Application {
    term: Terminal<CrosstermBackend<Stdout>>,
    current_screen: Box<dyn Screen>,
    router: Router,
}

impl Application {
    pub async fn new(
        db: Pool<ConnectionManager<SqliteConnection>>,
        bitcoind_client: Client,
    ) -> Result<Self> {
        let term = setup_terminal()?;

        let node_manager = NodeManager::new(db.clone(), Arc::new(bitcoind_client)).await;
        let node_manager = Arc::new(Mutex::new(node_manager));

        let current_screen = HomeScreen::new();
        let router = Router::new(node_manager.clone());

        Ok(Self {
            term,
            router,
            current_screen: Box::new(current_screen),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        let inputs = match self.init_event_channel() {
            Ok(inputs) => inputs,
            Err(err) => return self.close().or(Err(err)),
        };

        loop {
            self.term.draw(|f| {
                let paint_future = self.current_screen.paint(f);
                block_on(paint_future);
            })?;

            let screen_event = match inputs.recv() {
                Ok(event) => match event {
                    AppEvent::Quit => break,
                    AppEvent::Back => Some(Action::Pop),
                    event => self.current_screen.handle_input(event).await?,
                },
                Err(err) => return self.close().or(Err(anyhow!(err))),
            };

            if let Some(event) = screen_event {
                self.current_screen = self.router.go_to(event, self.current_screen);
            }
        }

        self.close()
    }

    fn init_event_channel(&self) -> Result<Receiver<AppEvent>> {
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
                            KeyCode::Char('q') => (AppEvent::Quit, true),
                            KeyCode::Esc => (AppEvent::Back, false),
                            _ => (AppEvent::Input(key), false),
                        };
                        tx.send(app_event).expect("can send events");
                        if exit {
                            return;
                        }
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if let Ok(_) = tx.send(AppEvent::Tick) {
                        last_tick = Instant::now();
                    }
                }
            }
        });

        Ok(rx)
    }

    fn close(self) -> Result<()> {
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
