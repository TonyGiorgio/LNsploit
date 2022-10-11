use crate::models::{node, NodeManager};
use crate::router::{Action, Location, Router};
use crate::screens::{AppEvent, InputMode, ParentScreen, Screen};
use crate::utility::clipboard::{get_clipboard_provider, ClipboardProvider, ClipboardType};
use crate::FilesystemLogger;
use anyhow::{anyhow, Result};
use bitcoincore_rpc::Client;
use crossterm::event::{KeyEvent, KeyModifiers, ModifierKeyCode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::SqliteConnection;
use futures::executor::block_on;
use lightning::util::logger::{Logger, Record};
use std::io::{self, Stdout};
use std::sync::{
    mpsc::{self, Receiver},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tui::{backend::CrosstermBackend, Terminal};

// Toggle the noisiest logs
const VERBOSE: bool = false;

pub struct AppState {
    pub node_manager: Arc<Mutex<NodeManager>>,
    pub router: Router,
    pub cached_nodes_list: Arc<Vec<String>>,
    pub logger: Arc<FilesystemLogger>,
    pub input_mode: InputMode,
    pub paste_contents: Option<Arc<String>>, // pub clipboard_provider: Arc<Box<dyn ClipboardProvider>>,
}

pub struct Application {
    term: Terminal<CrosstermBackend<Stdout>>,
}

impl Application {
    pub async fn new() -> Result<Self> {
        let term = setup_terminal()?;

        Ok(Self { term })
    }

    pub async fn run(
        mut self,
        db: Pool<ConnectionManager<SqliteConnection>>,
        bitcoind_client: Client,
        logger: Arc<FilesystemLogger>,
    ) -> Result<()> {
        let inputs = match self.init_event_channel() {
            Ok(inputs) => inputs,
            Err(err) => return self.close().or(Err(err)),
        };

        let node_manager =
            NodeManager::new(db.clone(), Arc::new(bitcoind_client), logger.clone()).await;
        let node_manager = Arc::new(Mutex::new(node_manager));

        let nodes_list = {
            let nodes = node_manager.clone().lock().await.list_nodes().await;
            nodes
                .iter()
                .map(|n| n.pubkey.clone())
                .collect::<Vec<String>>()
        };

        let mut state = AppState {
            node_manager,
            router: Router::new(),
            cached_nodes_list: Arc::new(nodes_list),
            logger: logger.clone(),
            input_mode: InputMode::Normal,
            paste_contents: None, // clipboard_provider: Arc::new(get_clipboard_provider()),
        };

        let mut screen = ParentScreen::new();

        loop {
            if VERBOSE {
                logger.log(&Record::new(
                    lightning::util::logger::Level::Debug,
                    format_args!(
                        "current route: {:?}, current active: {:?}, current stack: {:?}",
                        state.router.get_current_route(),
                        state.router.get_active_block(),
                        state.router.get_stack()
                    ),
                    "application",
                    "",
                    0,
                ));
            }

            self.term.draw(|f| {
                if VERBOSE {
                    logger.log(&Record::new(
                        lightning::util::logger::Level::Debug,
                        format_args!("about to paint scrren"),
                        "application",
                        "",
                        0,
                    ));
                }

                let paint_future = screen.paint(f, &state);
                block_on(paint_future);
                if VERBOSE {
                    logger.log(&Record::new(
                        lightning::util::logger::Level::Debug,
                        format_args!("got passed paint screen future"),
                        "application",
                        "",
                        0,
                    ));
                }
            })?;

            let screen_event = match inputs.recv() {
                Ok(event) => match event {
                    AppEvent::Quit => {
                        // do not allow in editing mode, pass q normally
                        if matches!(state.input_mode, InputMode::Editing) {
                            state.input_mode = InputMode::Normal;
                            screen
                                .handle_input(
                                    AppEvent::Input(KeyEvent::new(
                                        KeyCode::Char('q'),
                                        KeyModifiers::NONE,
                                    )),
                                    &mut state,
                                )
                                .await?;
                        }

                        logger.log(&Record::new(
                            lightning::util::logger::Level::Debug,
                            format_args!("handling quit event"),
                            "application",
                            "",
                            0,
                        ));
                        // state.navigation_stack.pop();
                        break;
                    }
                    AppEvent::Back => {
                        // if input state is editing, move to normal
                        if matches!(state.input_mode, InputMode::Editing) {
                            state.input_mode = InputMode::Normal;
                        }

                        // let the screens attempt to handle this
                        logger.log(&Record::new(
                            lightning::util::logger::Level::Debug,
                            format_args!("handling back event"),
                            "application",
                            "",
                            0,
                        ));

                        let screen_event = screen
                            .handle_input(
                                AppEvent::Input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                                &mut state,
                            )
                            .await?;

                        // let's clear the clipboard state as well otherwise it's dumb
                        state.paste_contents = None;

                        match screen_event {
                            Some(event) => {
                                logger.log(&Record::new(
                                    lightning::util::logger::Level::Debug,
                                    format_args!("got an event back from screen: {:?}", event),
                                    "application",
                                    "",
                                    0,
                                ));
                                Some(event)
                            }
                            None => None, // TODO consider letting this override screen
                        }
                    }
                    AppEvent::Paste => {
                        let clipboard = get_clipboard_provider();
                        let paste = clipboard.get_contents(ClipboardType::Clipboard);
                        logger.log(&Record::new(
                            lightning::util::logger::Level::Debug,
                            format_args!("paste result: {:?}", paste),
                            "application",
                            "",
                            0,
                        ));

                        if let Ok(paste) = paste {
                            state.paste_contents = Some(Arc::new(paste.trim().into()));
                        }

                        None
                    }
                    event => {
                        if VERBOSE {
                            logger.log(&Record::new(
                                lightning::util::logger::Level::Debug,
                                format_args!(
                                    "passing event ({:?}) to screen: {:?}",
                                    event, screen.menu_index
                                ),
                                "application",
                                "",
                                0,
                            ));
                        }

                        screen.handle_input(event, &mut state).await?
                    }
                },
                Err(err) => {
                    logger.log(&Record::new(
                        lightning::util::logger::Level::Error,
                        format_args!("error with screen event input: {}", err),
                        "application",
                        "",
                        0,
                    ));
                    return self.close().or(Err(anyhow!(err)));
                }
            };

            if let Some(event) = screen_event {
                logger.log(&Record::new(
                    lightning::util::logger::Level::Debug,
                    format_args!("screen event: {:?}", event),
                    "application",
                    "",
                    0,
                ));
                state.router.go_to(event);
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
                        let (app_event, exit) = match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) => (AppEvent::Back, false),
                            (KeyCode::Char('q'), _) => (AppEvent::Quit, false),
                            (KeyCode::Char('c'), KeyModifiers::CONTROL) => (AppEvent::Copy, false),
                            (KeyCode::Char('v'), KeyModifiers::CONTROL) => (AppEvent::Paste, false),
                            _ => (AppEvent::Input(key), false),
                        };
                        tx.send(app_event).expect("can send events");
                        // TODO: we have to pass the quit event, so we don't get to clean up this thread here anymore
                        // if exit {
                        //     return;
                        // }
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
