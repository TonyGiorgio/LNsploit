use super::schema::nodes;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Node {
    pub id: i32,
    pub pubkey: String,
}

#[derive(Insertable)]
#[diesel(table_name = nodes)]
pub struct NewNode<'a> {
    pub pubkey: &'a str,
}
