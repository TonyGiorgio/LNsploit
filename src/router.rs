#[derive(Clone, Debug, PartialEq)]
pub enum Location {
    Home,
    NodesList,
    Node(String),
    Simulation,
}

#[derive(Debug, Clone)]
pub enum Action {
    Push(Location),
    Replace(Location),
    Pop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActiveBlock {
    NoneBlock,
    Menu,
    Nodes,
    Main(Location),
}

#[derive(Clone, Debug, PartialEq)]
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
        let (next_route, next_block) = match action {
            Action::Push(location) => {
                self.screen_stack.push(location.clone());

                (location.clone(), location_to_active_block(location.clone()))
            }
            Action::Replace(location) => {
                (location.clone(), location_to_active_block(location.clone()))
            }
            Action::Pop => {
                let location = self.screen_stack.pop().unwrap_or(self.active_route.clone());

                (location.clone(), location_to_active_block(location.clone()))
            }
        };

        self.active_route = next_route;
        self.active_block = next_block
    }

    pub fn get_current_route(&self) -> &Location {
        &self.active_route
    }

    pub fn get_active_block(&self) -> &ActiveBlock {
        &self.active_block
    }
}

pub fn location_to_active_block(loc: Location) -> ActiveBlock {
    match loc {
        Location::Node(n) => ActiveBlock::Main(Location::Node(n)),
        Location::Simulation => ActiveBlock::Main(Location::Simulation),
        _ => ActiveBlock::NoneBlock,
    }
}
