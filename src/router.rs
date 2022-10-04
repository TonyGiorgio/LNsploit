use crate::models::NodeManager;
use crate::screens::{HomeScreen, NodeScreen, NodesListScreen, Screen};
use std::sync::Arc;
use tokio::sync::Mutex;

pub enum Location {
    Home,
    NodesList,
    Node(String),
}

pub enum Action {
    Push(Location),
    Replace(Location),
    Pop,
}

pub struct Router {
    screen_stack: Vec<Box<dyn Screen>>,
    node_manager: Arc<Mutex<NodeManager>>,
}

impl Router {
    pub fn new(node_manager: Arc<Mutex<NodeManager>>) -> Self {
        let screen_stack = vec![];
        Self {
            screen_stack,
            node_manager,
        }
    }

    pub fn go_to(&mut self, action: Action, current: Box<dyn Screen>) -> Box<dyn Screen> {
        match action {
            Action::Push(location) => {
                self.screen_stack.push(current);
                self.route_to_screen(location)
            }
            Action::Replace(location) => self.route_to_screen(location),
            Action::Pop => self.screen_stack.pop().unwrap_or(current),
        }
    }

    fn route_to_screen(&mut self, location: Location) -> Box<dyn Screen> {
        match location {
            Location::Home => Box::new(HomeScreen::new()),
            Location::NodesList => Box::new(NodesListScreen::new(self.node_manager.clone())),
            Location::Node(pubkey) => Box::new(NodeScreen::new(self.node_manager.clone(), pubkey)),
        }
    }
}
