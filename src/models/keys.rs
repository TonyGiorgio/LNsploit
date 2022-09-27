use super::schema::{master_keys, node_keys};
use diesel::prelude::*;

#[derive(Queryable)]
pub struct MasterKey {
    pub id: String,
    pub seed: Vec<u8>,
    pub mnemonic: String,
}

#[derive(Insertable)]
#[diesel(table_name = master_keys)]
pub struct NewMasterKey<'a> {
    pub id: &'a str,
    pub seed: Vec<u8>,
    pub mnemonic: &'a str,
}

#[derive(Queryable)]
pub struct NodeKey {
    pub id: String,
    pub master_key_id: String,
    pub child_index: i32,
}

#[derive(Insertable)]
#[diesel(table_name = node_keys)]
pub struct NewNodeKey<'a> {
    pub id: &'a str,
    pub master_key_id: &'a str,
    pub child_index: i32,
}
