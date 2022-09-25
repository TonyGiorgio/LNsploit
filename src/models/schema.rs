// @generated automatically by Diesel CLI.

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
        key_id -> Text,
        node_id -> Text,
    }
}

diesel::table! {
    nodes (id) {
        id -> Text,
        pubkey -> Text,
    }
}

diesel::joinable!(node_keys -> master_keys (key_id));
diesel::joinable!(node_keys -> nodes (node_id));

diesel::allow_tables_to_appear_in_same_query!(
    master_keys,
    node_keys,
    nodes,
);
