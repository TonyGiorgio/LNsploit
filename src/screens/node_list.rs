use crate::{
    application::AppState,
    router::{Action, ActiveBlock, Location},
    screens::MenuItemData,
};

pub fn handle_enter_node_list(
    state: &mut AppState,
) -> (Option<Action>, Option<Vec<(String, MenuItemData)>>) {
    // if the current active block is node list then do nothing
    if state.router.get_active_block() == &ActiveBlock::Nodes {
        return (None, None);
    }

    // set menu list to node list with the associated data
    let new_list = state
        .cached_nodes_list
        .iter()
        .map(|x| (String::from(x), MenuItemData::NodePubkey(String::from(x))))
        .collect::<Vec<(String, MenuItemData)>>();

    (Some(Action::Replace(Location::NodesList)), Some(new_list))
}
