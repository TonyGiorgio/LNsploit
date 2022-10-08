#[derive(Clone, Debug)]
pub enum Location {
    Home,
    NodesList,
    Node(String),
    Simulation,
}

#[derive(Debug)]
pub enum Action {
    Push(Location),
    Replace(Location),
    Pop,
}

pub enum ActiveBlock {
    Menu,
    Nodes,
    Main(Location),
}

pub enum HoveredBlock {
    Menu,
    Nodes,
}

pub struct Router {
    screen_stack: Vec<Location>,
    active_route: Location,
    active_block: ActiveBlock,
    hovered_block: HoveredBlock,
}

impl Router {
    pub fn new() -> Self {
        let screen_stack = vec![];
        Self {
            screen_stack,
            active_route: Location::Home,
            active_block: ActiveBlock::Menu,
            hovered_block: HoveredBlock::Menu,
        }
    }

    pub fn go_to(&mut self, action: Action) {
        let next = match action {
            Action::Push(location) => {
                self.screen_stack.push(location.clone());
                location
            }
            Action::Replace(location) => location,
            Action::Pop => self.screen_stack.pop().unwrap_or(self.active_route.clone()),
        };

        self.active_route = next;
    }

    pub fn get_current_route(&self) -> &Location {
        &self.active_route
    }
}
