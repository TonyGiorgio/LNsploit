use super::{MasterKey, NewMasterKey, NewNode, Node};

use super::schema::master_keys::dsl::*;
use super::schema::nodes::dsl::*;
use bip39::Mnemonic;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::SqliteConnection;
use uuid::Uuid;

pub struct NodeManager {
    db: Pool<ConnectionManager<SqliteConnection>>,
}

impl NodeManager {
    pub async fn new(db: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        let mut node_manager = Self { db };

        // check to make sure at least one master key has been initialized
        node_manager.check_keys();

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
        let conn = &mut self.db.get().unwrap();

        // TODO create the node properly
        let node_id = Uuid::new_v4().to_string();
        let new_node = NewNode {
            id: String::as_str(&node_id),
            pubkey: name.as_str(),
        };
        diesel::insert_into(nodes)
            .values(&new_node)
            .execute(conn)
            .expect("Error saving new node"); // TODO do not panic here
    }

    /// check_keys will check that a master key has been set up
    /// and if not, will create a new master key.
    fn check_keys(&mut self) {
        let conn = &mut self.db.get().unwrap();
        let master_key_list = master_keys
            .load::<MasterKey>(conn)
            .expect("Error loading master keys");
        if master_key_list.len() > 0 {
            return;
        }

        // if no master keys, create one
        let mnemonic_key = Mnemonic::generate(24).expect("could not create random mnemonic");
        let master_key_id = Uuid::new_v4().to_string();
        let phrase = mnemonic_key.to_string();
        let new_master_key = NewMasterKey {
            id: String::as_str(&master_key_id),
            seed: mnemonic_key.to_seed("").to_vec(),
            mnemonic: String::as_str(&phrase),
        };
        diesel::insert_into(master_keys)
            .values(&new_master_key)
            .execute(conn)
            .expect("Error creating new master key");
    }
}
