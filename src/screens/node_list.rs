use crate::{
    application::AppState,
    router::{Action, ActiveBlock, Location},
};

// this is the entry point for setting the node pubkey list as the active
pub fn handle_enter_node_list(state: &mut AppState) -> (Option<Action>, Option<Vec<String>>) {
    // if the current active block is node list then do nothing
    if state.router.get_active_block() == &ActiveBlock::Nodes {
        return (None, None);
    }

    // set menu list to node list
    let new_list = state
        .cached_nodes_list
        .iter()
        .map(String::from)
        .collect::<Vec<String>>();

    (Some(Action::Replace(Location::NodesList)), Some(new_list))
}
