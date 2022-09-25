use super::NewNode;
use super::Node;

use super::schema::nodes::dsl::*;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::SqliteConnection;

pub struct NodeManager {
    db: Pool<ConnectionManager<SqliteConnection>>,
}

impl NodeManager {
    pub async fn new(db: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        let mut node_manager = Self { db };

        // TODO do not create a new node each time it loads
        // these now save to DB
        node_manager.new_node(String::from("test"));

        node_manager
    }

    pub async fn list_nodes(&mut self) -> Vec<Node> {
        let conn = &mut self.db.get().unwrap();
        let node_list = nodes.load::<Node>(conn).expect("Error loading nodes"); // TODO do not panic
        node_list
    }

    pub fn new_node(&mut self, name: String) {
        // TODO pubkey should not be passed in like this
        let new_node = NewNode {
            pubkey: String::as_str(&name),
        };

        let conn = &mut self.db.get().unwrap();
        diesel::insert_into(nodes)
            .values(&new_node)
            .execute(conn)
            .expect("Error saving new post"); // TODO do not panic here
    }
}
