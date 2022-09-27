use super::schema::master_keys::dsl::*;
use super::schema::node_keys::dsl::*;
use super::schema::nodes::dsl::*;
use super::{MasterKey, NewMasterKey, NewNode, NewNodeKey, Node, NodeKey, RunnableNode};
use bip32::Mnemonic;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use rand_core::OsRng;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct NodeManager {
    db: Pool<ConnectionManager<SqliteConnection>>,
    nodes: Arc<Mutex<Vec<RunnableNode>>>,
}

impl NodeManager {
    pub async fn new(db: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        let mut node_manager = Self {
            db,
            nodes: Arc::new(Mutex::new(vec![])),
        };

        // check to make sure at least one master key has been initialized
        node_manager.check_keys();

        // TODO do not create a new node each time it loads
        // these now save to DB
        let _create_node_res = node_manager.new_node(String::from("test")).await;

        node_manager
    }

    pub async fn list_nodes(&mut self) -> Vec<Node> {
        let conn = &mut self.db.get().unwrap();
        let node_list = nodes.load::<Node>(conn).expect("Error loading nodes"); // TODO do not panic
        node_list
    }

    pub async fn new_node(&mut self, name: String) -> Result<(), Box<dyn std::error::Error>> {
        let conn = &mut self.db.get().unwrap();

        // First get the last child index that was used to create a node
        let mut next_child_index = 0;
        let node_key_list = node_keys
            .limit(1)
            .order(child_index.desc())
            .load::<NodeKey>(conn)
            .expect("Error loading node keys"); // TODO do not panic
        if node_key_list.len() > 0 {
            next_child_index = node_key_list[0].child_index + 1;
        }

        // retrieve the master key
        let master_key_list = master_keys
            .limit(1) // right now only ever plan on having one
            .load::<MasterKey>(conn)
            .expect("Error loading master key"); // TODO do not panic
        if master_key_list.len() < 1 {
            panic!("there is no master key loaded");
        }
        let master_key_id_ref = master_key_list[0].id.clone();

        // create a new node key
        let new_node_key_id = Uuid::new_v4().to_string();
        let new_node_key = NewNodeKey {
            id: String::as_str(&new_node_key_id),
            master_key_id: String::as_str(&master_key_id_ref),
            child_index: next_child_index,
        };
        diesel::insert_into(node_keys)
            .values(&new_node_key)
            .execute(conn)
            .expect("Error saving new node"); // TODO do not panic here

        // create the new node
        let new_node_id = Uuid::new_v4().to_string();
        let new_node = NewNode {
            id: String::as_str(&new_node_id),
            pubkey: name.as_str(),
            key_id: String::as_str(&new_node_key_id),
        };
        diesel::insert_into(nodes)
            .values(&new_node)
            .execute(conn)
            .expect("Error saving new node"); // TODO do not panic here

        // now start the node that was created
        let runnable_node = RunnableNode::new(self.db.clone(), new_node_id, name, new_node_key_id);
        match runnable_node {
            Ok(runnable_node) => {
                self.nodes.lock().await.push(runnable_node);
                Ok(())
            }
            Err(err) => Err(err),
        }
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
        let mnemonic_key = Mnemonic::random(&mut OsRng, Default::default());
        let new_master_key_id = Uuid::new_v4().to_string();
        let new_master_key = NewMasterKey {
            id: String::as_str(&new_master_key_id),
            seed: mnemonic_key.to_seed("").as_bytes().to_vec(),
            mnemonic: mnemonic_key.phrase(),
        };
        diesel::insert_into(master_keys)
            .values(&new_master_key)
            .execute(conn)
            .expect("Error creating new master key");
    }
}
