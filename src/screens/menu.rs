use std::{fmt, str::FromStr};

use crate::{
    application::AppState,
    router::{Action, ActiveBlock, Location},
};

#[derive(Default)]
pub enum MenuAction {
    Home,
    NetworkView,
    Routing,
    Exploits,
    SimulationMode,
    #[default]
    Invalid,
}

impl fmt::Display for MenuAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MenuAction::Home => write!(f, "Home"),
            MenuAction::NetworkView => write!(f, "Network View"),
            MenuAction::Routing => write!(f, "Routing"),
            MenuAction::Exploits => write!(f, "Exploits"),
            MenuAction::SimulationMode => write!(f, "Simulation Mode"),
            MenuAction::Invalid => write!(f, "Invalid"),
        }
    }
}

pub(crate) const MAIN_MENU: [MenuAction; 5] = [
    MenuAction::Home,
    MenuAction::NetworkView,
    MenuAction::Routing,
    MenuAction::Exploits,
    MenuAction::SimulationMode,
];

impl FromStr for MenuAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Home" => Ok(MenuAction::Home),
            "Network View" => Ok(MenuAction::NetworkView),
            "Routing" => Ok(MenuAction::Routing),
            "Exploits" => Ok(MenuAction::Exploits),
            "Simulation Mode" => Ok(MenuAction::SimulationMode),
            _ => Ok(MenuAction::Invalid),
        }
    }
}

// this is the entry point for setting the menu list as the active
pub(crate) fn handle_enter_main_menu(
    state: &mut AppState,
) -> (Option<Action>, Option<Vec<String>>) {
    // if the current active block is node list then do nothing
    if state.router.get_active_block() == &ActiveBlock::Menu {
        return (None, None);
    }

    // set menu list to menu items
    let new_list = MAIN_MENU
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    (Some(Action::Replace(Location::Home)), Some(new_list))
}

// this is an action taken on the menu list item
pub(crate) fn handle_enter_main(
    _state: &mut AppState,
    menu_action: MenuAction,
) -> (Option<Action>, Option<Vec<String>>) {
    let action = match menu_action {
        MenuAction::SimulationMode => Action::Push(Location::Simulation),
        MenuAction::Exploits => Action::Push(Location::Exploits),
        _ => return (None, None),
    };

    (Some(action), None)
}
