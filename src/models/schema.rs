// @generated automatically by Diesel CLI.

diesel::table! {
    channel_monitors (id) {
        id -> Text,
        node_id -> Text,
        channel_tx_id -> Text,
        channel_tx_index -> Integer,
        channel_monitor_data -> Binary,
        original_channel_monitor_data -> Binary,
    }
}

diesel::table! {
    channel_updates (id) {
        id -> Text,
        node_id -> Text,
        channel_tx_id -> Text,
        channel_tx_index -> Integer,
        channel_internal_update_id -> Integer,
        channel_update_data -> Binary,
    }
}

diesel::table! {
    key_values (key_value_id) {
        key_value_id -> Text,
        id -> Text,
        node_id -> Text,
        data_value -> Binary,
    }
}

diesel::table! {
    master_keys (id) {
        id -> Text,
        seed -> Binary,
        mnemonic -> Text,
    }
}

diesel::table! {
    node_keys (id) {
        id -> Text,
        master_key_id -> Text,
        child_index -> Integer,
    }
}

diesel::table! {
    nodes (id) {
        id -> Text,
        pubkey -> Text,
        key_id -> Text,
    }
}

diesel::joinable!(channel_monitors -> nodes (node_id));
diesel::joinable!(channel_updates -> nodes (node_id));
diesel::joinable!(node_keys -> master_keys (master_key_id));
diesel::joinable!(nodes -> node_keys (key_id));

diesel::allow_tables_to_appear_in_same_query!(
    channel_monitors,
    channel_updates,
    key_values,
    master_keys,
    node_keys,
    nodes,
);
