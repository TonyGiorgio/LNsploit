use super::schema::master_keys::dsl::*;
use super::schema::node_keys::dsl::*;
use super::schema::nodes;
use super::{MasterKey, NodeKey};
use bip32::{Mnemonic, XPrv};
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use diesel::{prelude::*, r2d2::ConnectionManager, r2d2::Pool};
use lightning::chain::keysinterface::{KeysInterface, KeysManager, Recipient};
use std::sync::Arc;
use std::time::SystemTime;

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

pub struct RunnableNode {
    db: Pool<ConnectionManager<SqliteConnection>>,
    pub db_id: String,
    pub pubkey: String,
    pub key_id: String,
    pub xpriv: XPrv,
    pub keys_manager: Arc<KeysManager>,
}

impl RunnableNode {
    pub fn new(
        db: Pool<ConnectionManager<SqliteConnection>>,
        db_id: String,
        key_id: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = &mut db.get().unwrap();

        // find the node key information
        let (node_child_index, node_master_key_id) =
            match node_keys.find(key_id.clone()).first::<NodeKey>(conn) {
                Ok(node_key) => (node_key.child_index, node_key.master_key_id),
                Err(_) => return Err("Cannot find node key")?,
            };

        // get the master private key for this node
        let master_mnemonic = match master_keys
            .find(node_master_key_id.clone())
            .first::<MasterKey>(conn)
        {
            Ok(master_private_key) => {
                Mnemonic::new(master_private_key.mnemonic, Default::default())
                    .expect("master seed phrase could not be parsed")
            }
            Err(_) => return Err("Cannot find master private key")?,
        };

        // derive the child private key from the master and save it in this struct
        let xpriv = XPrv::new(&master_mnemonic.to_seed(""))?
            .derive_child(bip32::ChildNumber(node_child_index as u32))?;

        // init KeysManager
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let keys_manager = Arc::new(KeysManager::new(
            &xpriv.to_bytes(),
            current_time.as_secs(),
            current_time.subsec_nanos(),
        ));

        // find out the pubkey
        let mut secp_ctx = Secp256k1::new();
        let keys_manager_clone = keys_manager.clone();
        secp_ctx.seeded_randomize(&keys_manager_clone.get_secure_random_bytes());
        let our_network_key = keys_manager_clone
            .get_node_secret(Recipient::Node)
            .expect("cannot parse node secret");
        let pubkey = PublicKey::from_secret_key(&secp_ctx, &our_network_key).to_string();

        return Ok(RunnableNode {
            db,
            db_id,
            pubkey,
            key_id,
            xpriv,
            keys_manager,
        });
    }
}
