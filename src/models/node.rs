use super::schema::master_keys::dsl::*;
use super::schema::node_keys::dsl::*;
use super::schema::nodes;
use super::{KVNodePersister, MasterKey, NodeKey, NodePersister};
use crate::FilesystemLogger;
use bip32::{Mnemonic, XPrv};
use bitcoin::blockdata::block::Block;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::hash_types::BlockHash;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::util::uint::Uint256;
use bitcoincore_rpc::bitcoincore_rpc_json::EstimateMode;
use bitcoincore_rpc::{Client, RpcApi};
use diesel::{prelude::*, r2d2::ConnectionManager, r2d2::Pool};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::chain::keysinterface::{InMemorySigner, KeysInterface, KeysManager, Recipient};
use lightning::chain::{self, Filter, Watch};
use lightning::chain::{chainmonitor, BestBlock};
use lightning::ln::channelmanager::{self, ChannelManagerReadArgs};
use lightning::ln::channelmanager::{ChainParameters, SimpleArcChannelManager};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler, SimpleArcPeerManager};
use lightning::onion_message::SimpleArcOnionMessenger;
use lightning::routing::gossip::{self, P2PGossipSync};
use lightning::util::config::UserConfig;
use lightning::util::logger::{Logger, Record};
use lightning::util::ser::ReadableArgs;
use lightning_block_sync::{
    poll, AsyncBlockSourceResult, BlockHeaderData, BlockSource, BlockSourceError, UnboundedCache,
};
use lightning_net_tokio::SocketDescriptor;
use rand::Rng;
use std::io::Cursor;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

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

#[derive(Clone)]
pub struct RunnableNode {
    db: Pool<ConnectionManager<SqliteConnection>>,
    pub db_id: String,
    pub pubkey: String,
    pub key_id: String,
    pub xpriv: XPrv,
    pub keys_manager: Arc<KeysManager>,
    pub persister: Arc<NodePersister>,
    pub ldk_bitcoind_client: Arc<LdkBitcoindClient>,
    pub logger: Arc<FilesystemLogger>,
}

impl RunnableNode {
    pub async fn new(
        db: Pool<ConnectionManager<SqliteConnection>>,
        db_id: String,
        key_id: String,
        bitcoind_client: Arc<Client>,
        logger: Arc<FilesystemLogger>,
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

        logger.log(&Record::new(
            lightning::util::logger::Level::Info,
            format_args!("Starting node {}", pubkey.clone()),
            "node",
            "",
            0,
        ));

        // init the LDK wrapper for bitcoind
        let ldk_bitcoind_client = Arc::new(LdkBitcoindClient { bitcoind_client });

        //initialize the fee estimator
        let fee_estimator = ldk_bitcoind_client.clone();

        // initialize the broadcaster interface
        let broadcaster = ldk_bitcoind_client.clone();

        // block source
        let block_source = ldk_bitcoind_client.clone();

        // create the persisters
        // one for general SQL and one for KV for general LDK values
        let persister = Arc::new(NodePersister::new(db.clone(), db_id.clone()));
        let kv_persister = Arc::new(KVNodePersister::new(db.clone(), db_id.clone()));

        // init chain monitor
        let chain_monitor: Arc<ChainMonitor> = Arc::new(chainmonitor::ChainMonitor::new(
            None,
            broadcaster.clone(),
            logger.clone(),
            fee_estimator.clone(),
            persister.clone(),
        ));

        // read channelmonitor state from disk
        let mut channelmonitors = persister
            .read_channelmonitors(keys_manager.clone())
            .unwrap();

        // Load channel monitor updates from disk as well
        let channelmonitorupdates = persister.read_channelmonitor_updates().unwrap();
        for (_, channel_monitor) in channelmonitors.iter_mut() {
            // which utxo is this channel monitoring for?
            let (channel_output, _) = channel_monitor.get_funding_txo();
            let channel_updates_res = channelmonitorupdates.get(&channel_output.txid);
            match channel_updates_res {
                Some(channel_updates) => {
                    // if we found the channel monitor for this channel update,
                    // apply in order
                    let mut sorted_channel_updates = channel_updates.clone();
                    sorted_channel_updates.sort_by(|a, b| a.update_id.cmp(&b.update_id));
                    for channel_monitor_update in sorted_channel_updates.iter_mut() {
                        logger.log(&Record::new(
                            lightning::util::logger::Level::Debug,
                            format_args!(
                                "applying update {} for {}",
                                channel_monitor_update.update_id, channel_output.txid
                            ),
                            "node",
                            "",
                            0,
                        ));

                        match channel_monitor.update_monitor(
                            channel_monitor_update,
                            &broadcaster,
                            fee_estimator.clone(),
                            &logger,
                        ) {
                            Ok(_) => continue,
                            Err(e) => {
                                panic!("could not process update monitor: {:?}", e)
                            }
                        }
                    }
                }
                None => continue,
            }
        }

        // init the channel manager

        let mut user_config = UserConfig::default();
        user_config
            .channel_handshake_limits
            .force_announced_channel_preference = false;
        let mut restarting_node = true;
        let (channel_manager_blockhash, channel_manager) = {
            let (already_init, kv_value) = match kv_persister.read_value("manager") {
                Ok(kv_value) => {
                    // check if kv value is filled or not
                    if kv_value.is_empty() {
                        (false, vec![])
                    } else {
                        (true, kv_value)
                    }
                }
                Err(_) => (false, vec![]),
            };

            if already_init {
                let mut channel_monitor_mut_references = Vec::new();
                for (_, channel_monitor) in channelmonitors.iter_mut() {
                    channel_monitor_mut_references.push(channel_monitor);
                }
                let read_args = ChannelManagerReadArgs::new(
                    keys_manager.clone(),
                    fee_estimator.clone(),
                    chain_monitor.clone(),
                    broadcaster.clone(),
                    logger.clone(),
                    user_config,
                    channel_monitor_mut_references,
                );
                let mut readable_kv_value = Cursor::new(kv_value);
                <(BlockHash, RunnableChannelManager)>::read(&mut readable_kv_value, read_args)
                    .unwrap()
            } else {
                // We're starting a fresh node.
                restarting_node = false;
                let getinfo_resp = ldk_bitcoind_client
                    .bitcoind_client
                    .get_blockchain_info()
                    .unwrap(); // TODO do not unwrap

                let chain_params = ChainParameters {
                    network: bitcoin::Network::Regtest, // TODO load
                    best_block: BestBlock::new(
                        getinfo_resp.best_block_hash,
                        getinfo_resp.blocks as u32,
                    ),
                };
                let fresh_channel_manager = channelmanager::ChannelManager::new(
                    fee_estimator.clone(),
                    chain_monitor.clone(),
                    broadcaster.clone(),
                    logger.clone(),
                    keys_manager.clone(),
                    user_config,
                    chain_params,
                );
                (getinfo_resp.best_block_hash, fresh_channel_manager)
            }
        };

        // sync to chain tip
        let mut chain_listener_channel_monitors = Vec::new();
        let mut cache = UnboundedCache::new();
        let mut chain_tip: Option<poll::ValidatedBlockHeader> = None;
        if restarting_node {
            let mut chain_listeners = vec![(
                channel_manager_blockhash,
                &channel_manager as &dyn chain::Listen,
            )];

            for (blockhash, channel_monitor) in channelmonitors.drain(..) {
                let outpoint = channel_monitor.get_funding_txo().0;
                chain_listener_channel_monitors.push((
                    blockhash,
                    (
                        channel_monitor,
                        broadcaster.clone(),
                        fee_estimator.clone(),
                        logger.clone(),
                    ),
                    outpoint,
                ));
            }

            for monitor_listener_info in chain_listener_channel_monitors.iter_mut() {
                chain_listeners.push((
                    monitor_listener_info.0,
                    &monitor_listener_info.1 as &dyn chain::Listen,
                ));
            }

            // TODO handle synchronize_listeners to catch up a restarting node
            // This is unsafe if blocks mine without this being on
            // May even crash, not sure. Having async problems...
            /*
                chain_tip = Some(
                    init::synchronize_listeners(
                        &mut block_source.deref(),
                        bitcoin::Network::Regtest, // TODO load
                        &mut cache,
                        chain_listeners,
                    )
                    .await
                    .unwrap(),
                );
            */
        }

        // give channel monitors to chain monitor
        for item in chain_listener_channel_monitors.drain(..) {
            let channel_monitor = item.1 .0;
            let funding_outpoint = item.2;
            chain_monitor
                .watch_channel(funding_outpoint, channel_monitor)
                .unwrap();
        }

        // initialize network graph
        let genesis = genesis_block(bitcoin::Network::Regtest).header.block_hash();
        let network_graph = Arc::new(kv_persister.read_network(genesis, logger.clone()));

        let gossip_sync = Arc::new(P2PGossipSync::new(
            Arc::clone(&network_graph),
            None::<Arc<dyn chain::Access + Send + Sync>>,
            logger.clone(),
        ));
        let network_graph_persist = Arc::clone(&network_graph);
        let network_graph_logger = logger.clone();
        tokio::spawn(async move {
            // wait for a little bit before trying to save..
            sleep(Duration::from_secs(5));
            let mut interval = tokio::time::interval(Duration::from_secs(600));
            loop {
                interval.tick().await;
                let res = kv_persister.persist_network(&network_graph_persist);
                if res.is_err() {
                    // Persistence errors here are non-fatal as we can just fetch the routing graph
                    // again later, but they may indicate a disk error which could be fatal elsewhere.
                    network_graph_logger.log(&Record::new(
                        lightning::util::logger::Level::Error,
                        format_args!("Failed to persist network graph to DB"),
                        "node",
                        "",
                        0,
                    ));
                }
            }
        });

        // initialize peer manager
        let channel_manager: Arc<RunnableChannelManager> = Arc::new(channel_manager);
        let onion_messenger: Arc<OnionMessenger> =
            Arc::new(OnionMessenger::new(keys_manager.clone(), logger.clone()));
        let mut ephemeral_bytes = [0; 32];
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        rand::thread_rng().fill_bytes(&mut ephemeral_bytes);
        let lightning_msg_handler = MessageHandler {
            chan_handler: channel_manager.clone(),
            route_handler: gossip_sync.clone(),
            onion_message_handler: onion_messenger.clone(),
        };
        let peer_manager: Arc<PeerManager> = Arc::new(PeerManager::new(
            lightning_msg_handler,
            keys_manager.get_node_secret(Recipient::Node).unwrap(),
            current_time,
            &ephemeral_bytes,
            logger.clone(),
            IgnoringMessageHandler {},
        ));

        return Ok(RunnableNode {
            db: db.clone(),
            db_id: db_id.clone(),
            persister,
            pubkey,
            key_id,
            xpriv,
            keys_manager,
            ldk_bitcoind_client,
            logger,
        });
    }
}

type ChainMonitor = chainmonitor::ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<LdkBitcoindClient>,
    Arc<LdkBitcoindClient>,
    Arc<FilesystemLogger>,
    Arc<NodePersister>,
>;

pub(crate) type NetworkGraph = gossip::NetworkGraph<Arc<FilesystemLogger>>;

pub(crate) type RunnableChannelManager =
    SimpleArcChannelManager<ChainMonitor, LdkBitcoindClient, LdkBitcoindClient, FilesystemLogger>;

pub(crate) type PeerManager = SimpleArcPeerManager<
    SocketDescriptor,
    ChainMonitor,
    LdkBitcoindClient,
    LdkBitcoindClient,
    dyn chain::Access + Send + Sync,
    FilesystemLogger,
>;

type OnionMessenger = SimpleArcOnionMessenger<FilesystemLogger>;

#[derive(Clone)]
pub struct LdkBitcoindClient {
    pub bitcoind_client: Arc<Client>,
}

impl BlockSource for &LdkBitcoindClient {
    fn get_header<'a>(
        &'a self,
        header_hash: &'a BlockHash,
        _height_hint: Option<u32>,
    ) -> AsyncBlockSourceResult<'a, BlockHeaderData> {
        Box::pin(async move {
            let res = self.bitcoind_client.get_block_header_info(header_hash);
            match res {
                Ok(res) => {
                    let converted_res = BlockHeaderData {
                        header: bitcoin::BlockHeader {
                            version: res.version,
                            prev_blockhash: res.previous_block_hash.unwrap(),
                            merkle_root: res.merkle_root,
                            time: res.time as u32,
                            bits: res.bits.parse::<u32>().unwrap(),
                            nonce: res.nonce,
                        },
                        height: res.height as u32,
                        chainwork: Uint256::from_be_bytes(res.chainwork.try_into().unwrap()),
                    };
                    Ok(converted_res)
                }
                // TODO verify error type
                Err(e) => Err(BlockSourceError::transient(e)),
            }
        })
    }

    fn get_block<'a>(&'a self, header_hash: &'a BlockHash) -> AsyncBlockSourceResult<'a, Block> {
        Box::pin(async move {
            let res = self.bitcoind_client.get_block(header_hash);
            match res {
                Ok(res) => Ok(res),
                // TODO verify error type
                Err(e) => Err(BlockSourceError::transient(e)),
            }
        })
    }

    fn get_best_block<'a>(&'a self) -> AsyncBlockSourceResult<(BlockHash, Option<u32>)> {
        Box::pin(async move {
            let res = self.bitcoind_client.get_blockchain_info();
            match res {
                Ok(res) => Ok((res.best_block_hash, Some(res.blocks as u32))),
                // TODO verify error type
                Err(e) => Err(BlockSourceError::transient(e)),
            }
        })
    }
}

const MIN_FEERATE: u32 = 253;

impl FeeEstimator for LdkBitcoindClient {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        match confirmation_target {
            ConfirmationTarget::Background => {
                let res = self
                    .bitcoind_client
                    .estimate_smart_fee(144, Some(EstimateMode::Economical));
                match res {
                    Ok(res) => {
                        if let Some(fee_rate) = res.fee_rate {
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat() / 4) as u32)
                        } else {
                            MIN_FEERATE
                        }
                    }
                    Err(_) => MIN_FEERATE,
                }
            }
            ConfirmationTarget::Normal => {
                let res = self
                    .bitcoind_client
                    .estimate_smart_fee(18, Some(EstimateMode::Conservative));
                match res {
                    Ok(res) => {
                        if let Some(fee_rate) = res.fee_rate {
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat() / 4) as u32)
                        } else {
                            // TODO probably not min for normal
                            MIN_FEERATE
                        }
                    }
                    // TODO probably not min for normal
                    Err(_) => MIN_FEERATE,
                }
            }
            ConfirmationTarget::HighPriority => {
                let res = self
                    .bitcoind_client
                    .estimate_smart_fee(6, Some(EstimateMode::Conservative));
                match res {
                    Ok(res) => {
                        if let Some(fee_rate) = res.fee_rate {
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat() / 4) as u32)
                        } else {
                            // TODO probably not min for high
                            MIN_FEERATE
                        }
                    }
                    // TODO probably not min for high
                    Err(_) => MIN_FEERATE,
                }
            }
        }
    }
}

impl BroadcasterInterface for LdkBitcoindClient {
    fn broadcast_transaction(&self, tx: &Transaction) {
        let res = self.bitcoind_client.send_raw_transaction(tx);
        // This may error due to RL calling `broadcast_transaction` with the same transaction
        // multiple times, but the error is safe to ignore.
        match res {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("Transaction already in block chain")
                    && !err_str.contains("Inputs missing or spent")
                    && !err_str.contains("bad-txns-inputs-missingorspent")
                    && !err_str.contains("txn-mempool-conflict")
                    && !err_str.contains("non-BIP68-final")
                    && !err_str.contains("insufficient fee, rejecting replacement ")
                {
                    panic!("{}", e);
                }
            }
        }
    }
}
