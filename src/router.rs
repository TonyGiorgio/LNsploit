#[derive(Clone, Debug)]
pub enum Location {
    Home,
    NodesList,
    Node(String),
    Simulation,
}

pub enum Action {
    Push(Location),
    Replace(Location),
    Pop,
}

pub struct Router {
    screen_stack: Vec<Location>,
    active_route: Location,
}

impl Router {
    pub fn new() -> Self {
        let screen_stack = vec![];
        Self {
            screen_stack,
            active_route: Location::Home,
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
