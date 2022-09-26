use super::schema::nodes;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Node {
    pub id: String,
    pub pubkey: String,
    pub key_id: String,
}

#[derive(Insertable, Default)]
#[diesel(table_name = nodes)]
pub struct NewNode<'a> {
    pub id: &'a str,
    pub pubkey: &'a str,
    pub key_id: &'a str,
}
