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

diesel::joinable!(node_keys -> master_keys (master_key_id));
diesel::joinable!(nodes -> node_keys (key_id));

diesel::allow_tables_to_appear_in_same_query!(
    master_keys,
    node_keys,
    nodes,
);
