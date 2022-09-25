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
    pub key_id: String,
    pub node_id: String,
}

#[derive(Insertable)]
#[diesel(table_name = node_keys)]
pub struct NewNodeKey<'a> {
    pub id: &'a str,
    pub key_id: &'a str,
    pub node_id: &'a str,
}
