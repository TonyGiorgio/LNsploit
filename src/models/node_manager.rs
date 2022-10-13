use super::schema::master_keys::dsl::*;
use super::schema::node_keys::dsl::*;
use super::schema::nodes::dsl::*;
use super::{
    broadcast_lnd_15_exploit, MasterKey, NewMasterKey, NewNode, NewNodeKey, Node, NodeKey,
    RunnableNode,
};
use crate::FilesystemLogger;
use bip32::Mnemonic;
use bitcoincore_rpc::Client;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::logger::{Logger, Record};
use rand_core::OsRng;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct NodeManager {
    db: Pool<ConnectionManager<SqliteConnection>>,
    nodes: HashMap<String, Arc<RunnableNode>>,
    bitcoind_client: Arc<Client>,
    logger: Arc<FilesystemLogger>,
}

impl NodeManager {
    pub async fn new(
        db: Pool<ConnectionManager<SqliteConnection>>,
        bitcoind_client: Arc<Client>,
        logger: Arc<FilesystemLogger>,
    ) -> Self {
        let mut node_manager = Self {
            db: db.clone(),
            bitcoind_client: bitcoind_client.clone(),
            nodes: HashMap::new(),
            logger: logger.clone(),
        };

        // check to make sure at least one master key has been initialized
        node_manager.check_keys();

        // start and store all the nodes in the list
        let node_list = node_manager.list_nodes().await;
        for node_item in node_list {
            let runnable_node_logger = logger.clone();
            let runnable_node = RunnableNode::new(
                db.clone(),
                node_item.id.clone(),
                node_item.key_id.clone(),
                bitcoind_client.clone(),
                runnable_node_logger.clone(),
            )
            .await
            .expect("could not start node"); // TODO do not panic
            node_manager
                .nodes
                .insert(node_item.id.clone(), Arc::new(runnable_node));
        }

        node_manager
    }

    pub async fn list_nodes(&self) -> Vec<Node> {
        let conn = &mut self.db.get().unwrap();
        let node_list = nodes.load::<Node>(conn).expect("Error loading nodes"); // TODO do not panic
        node_list
    }

    pub async fn get_node_id_by_pubkey(&self, passed_pubkey: &str) -> Option<String> {
        for node_item in self.list_nodes().await {
            if node_item.pubkey == passed_pubkey {
                let db_id = node_item.id.clone();
                return Some(db_id);
            }
        }

        return None;
    }

    pub async fn connect_peer(
        &mut self,
        node_id: String,
        peer_connection_string: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.connect_peer(String::from(peer_connection_string))
            .await
    }

    pub fn list_channels(&mut self, node_id: String) -> Vec<ChannelDetails> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.list_channels()
    }

    pub fn list_peers(&mut self, node_id: String) -> Vec<String> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.list_peers()
    }

    pub fn create_invoice(
        &mut self,
        node_id: String,
        amount_sat: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.create_invoice(amount_sat)
    }

    pub fn pay_invoice(
        &mut self,
        node_id: String,
        invoice: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.pay_invoice(invoice)
    }

    pub async fn open_channel(
        &mut self,
        node_id: String,
        peer_pubkey: String,
        amount_sat: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.open_channel(peer_pubkey, amount_sat).await
    }

    pub async fn close_channel(
        &mut self,
        node_id: String,
        channel_id: String,
        peer_pubkey: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.close_channel(channel_id, peer_pubkey).await
    }

    pub async fn force_close_channel_with_initial_state(
        &mut self,
        node_id: String,
        channel_id: String,
        peer_pubkey: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.force_close_channel_with_initial_state(channel_id, peer_pubkey)
            .await
    }

    pub fn create_address(
        &mut self,
        node_id: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        node.create_address()
    }

    pub async fn new_node(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
        let runnable_node = RunnableNode::new(
            self.db.clone(),
            new_node_id.clone(),
            new_node_key_id.clone(),
            self.bitcoind_client.clone(),
            self.logger.clone(),
        )
        .await?;

        let new_node = NewNode {
            id: String::as_str(&new_node_id),
            pubkey: String::as_str(&runnable_node.pubkey),
            key_id: String::as_str(&new_node_key_id),
        };

        diesel::insert_into(nodes)
            .values(&new_node)
            .execute(conn)
            .expect("Error saving new node"); // TODO do not panic here

        self.nodes
            .insert(new_node_id.clone(), Arc::new(runnable_node));

        self.setup_node(new_node_id.clone());

        Ok(())
    }

    fn setup_node(&mut self, node_id: String) {
        let node = self.nodes.get(&node_id.clone()).expect("node is missing");

        // when a new node is created, create the bitcoind wallet for it
        node.create_wallet()
            .expect("could not create bitcoind wallet for node");
    }

    pub fn broadcast_lnd_15_exploit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match broadcast_lnd_15_exploit(self.bitcoind_client.clone()) {
            Ok(txid) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("broadcasted id: {}", txid),
                    "node",
                    "",
                    0,
                ));
                Ok(())
            }
            Err(e) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("could not broadcast exploit : {}", e),
                    "node",
                    "",
                    0,
                ));
                Err(e)
            }
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
