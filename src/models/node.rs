use super::schema::master_keys::dsl::*;
use super::schema::node_keys::dsl::*;
use super::schema::nodes;
use super::{KVNodePersister, LdkBitcoindClient, MasterKey, NodeKey, NodePersister};
use crate::FilesystemLogger;
use bip32::{Mnemonic, XPrv};
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::consensus::encode;
use bitcoin::hash_types::BlockHash;
use bitcoin::hashes::Hash;
use bitcoin::network::constants::Network;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Amount;
use bitcoin_bech32::WitnessProgram;
use bitcoincore_rpc::{Client, RpcApi};
use diesel::{prelude::*, r2d2::ConnectionManager, r2d2::Pool};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::chain::keysinterface::{InMemorySigner, KeysInterface, KeysManager, Recipient};
use lightning::chain::{self, Filter, Watch};
use lightning::chain::{chainmonitor, BestBlock};
use lightning::ln::channelmanager::{self, ChannelDetails, ChannelManagerReadArgs};
use lightning::ln::channelmanager::{ChainParameters, SimpleArcChannelManager};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler, SimpleArcPeerManager};
use lightning::ln::{PaymentHash, PaymentPreimage, PaymentSecret};
use lightning::onion_message::SimpleArcOnionMessenger;
use lightning::routing::gossip::{self, NodeId, P2PGossipSync};
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::util::config::{ChannelHandshakeConfig, ChannelHandshakeLimits, UserConfig};
use lightning::util::events::{Event, EventHandler, PaymentPurpose};
use lightning::util::logger::{Logger, Record};
use lightning::util::ser::ReadableArgs;
use lightning_background_processor::{BackgroundProcessor, GossipSync};
use lightning_block_sync::init;
use lightning_block_sync::SpvClient;
use lightning_block_sync::{poll, UnboundedCache};
use lightning_invoice::payment;
use lightning_invoice::payment::PaymentError;
use lightning_invoice::utils::DefaultRouter;
use lightning_invoice::{utils, Currency, Invoice};
use lightning_net_tokio::SocketDescriptor;
use rand::Rng;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::net::{SocketAddr, ToSocketAddrs};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
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
    pub invoice_payer: Arc<InvoicePayer<LdkEventHandler>>,
    pub peer_manager: Arc<PeerManager>,
    pub channel_manager: Arc<RunnableChannelManager>,
    pub network_graph: Arc<NetworkGraph>,
    pub onion_messenger: Arc<OnionMessenger>,
    inbound_payments: PaymentInfoStorage,
    outbound_payments: PaymentInfoStorage,
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
        let kv_persister_network_graph = kv_persister.clone();
        let network_graph =
            Arc::new(kv_persister_network_graph.read_network(genesis, logger.clone()));

        let gossip_sync = Arc::new(P2PGossipSync::new(
            Arc::clone(&network_graph),
            None::<Arc<dyn chain::Access + Send + Sync>>,
            logger.clone(),
        ));
        let network_graph_persist = Arc::clone(&network_graph);
        let network_graph_logger = logger.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600));
            loop {
                interval.tick().await;
                let res = kv_persister_network_graph.persist_network(&network_graph_persist);
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

        // init networking
        let peer_manager_connection_handler = peer_manager.clone();
        // generate random port number because who cares
        let listening_port: i32 = rand::thread_rng().gen_range::<i32>(1000, 65535);
        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", listening_port))
                .await
                .expect(
                    "Failed to bind to listen port - is something else already listening on it?",
                );
            loop {
                let peer_mgr = peer_manager_connection_handler.clone();
                let tcp_stream = listener.accept().await.unwrap().0;
                tokio::spawn(async move {
                    lightning_net_tokio::setup_inbound(
                        peer_mgr.clone(),
                        tcp_stream.into_std().unwrap(),
                    )
                    .await;
                });
            }
        });

        // connect and disconnect blocks
        let validate_block_header_source = ldk_bitcoind_client.clone();
        if chain_tip.is_none() {
            chain_tip = Some(
                init::validate_best_block_header(&mut validate_block_header_source.deref())
                    .await
                    .unwrap(),
            );
        }
        let channel_manager_listener = channel_manager.clone();
        let chain_monitor_listener = chain_monitor.clone();
        let bitcoind_block_source = ldk_bitcoind_client.clone();
        let network = bitcoin::Network::Regtest;
        tokio::spawn(async move {
            let mut derefed = bitcoind_block_source.deref();
            let chain_poller = poll::ChainPoller::new(&mut derefed, network);
            let chain_listener = (chain_monitor_listener, channel_manager_listener);
            let mut spv_client = SpvClient::new(
                chain_tip.unwrap(),
                chain_poller,
                &mut cache,
                &chain_listener,
            );
            loop {
                spv_client.poll_best_tip().await.unwrap();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        // handle ldk events
        let channel_manager_event_listener = channel_manager.clone();
        let keys_manager_listener = keys_manager.clone();
        let inbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
        let outbound_payments: PaymentInfoStorage = Arc::new(Mutex::new(HashMap::new()));
        let inbound_pmts_for_events = inbound_payments.clone();
        let outbound_pmts_for_events = outbound_payments.clone();
        let network = bitcoin::Network::Regtest;
        let event_handler_bitcoind = ldk_bitcoind_client.clone();
        let network_graph_events = network_graph.clone();
        let event_handler_logger = logger.clone();
        let event_handler = LdkEventHandler::new(
            channel_manager_event_listener,
            event_handler_bitcoind,
            network_graph_events,
            keys_manager_listener,
            inbound_pmts_for_events,
            outbound_pmts_for_events,
            network,
            event_handler_logger,
        );

        // init routing scorer
        let kv_persister_scorer = kv_persister.clone();
        let scorer = Arc::new(Mutex::new(
            kv_persister_scorer.read_scorer(Arc::clone(&network_graph), logger.clone()),
        ));
        let scorer_persist = Arc::clone(&scorer);
        let scorer_logger = logger.clone();
        // TODO consider moving this to the background runner
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600));
            loop {
                interval.tick().await;
                let locked_scorer_persist = scorer_persist.lock().unwrap();
                let res = kv_persister_scorer.persist_scroer(&locked_scorer_persist);
                if res.is_err() {
                    // Persistence errors here are non-fatal as we can just fetch the routing graph
                    // again later, but they may indicate a disk error which could be fatal elsewhere.
                    scorer_logger.log(&Record::new(
                        lightning::util::logger::Level::Error,
                        format_args!("Failed to persist scorer to DB"),
                        "node",
                        "",
                        0,
                    ));
                }
            }
        });

        // create invoice payer
        let router = DefaultRouter::new(
            network_graph.clone(),
            logger.clone(),
            keys_manager.get_secure_random_bytes(),
        );
        let invoice_payer = Arc::new(InvoicePayer::new(
            channel_manager.clone(),
            router,
            scorer.clone(),
            logger.clone(),
            event_handler,
            payment::Retry::Timeout(Duration::from_secs(0)), // No ever trying to retry payments
        ));

        let background_processor_logger = logger.clone();
        let background_processor_pubkey = pubkey.clone();
        let background_processor_invoice_payer = invoice_payer.clone();
        let background_processor_peer_manager = peer_manager.clone();
        let background_processor_channel_manager = channel_manager.clone();
        tokio::spawn(async move {
            background_processor_logger.log(&Record::new(
                lightning::util::logger::Level::Info,
                format_args!(
                    "starting background processor for node: {}",
                    background_processor_pubkey.clone()
                ),
                "node",
                "",
                0,
            ));

            let _background_processor = BackgroundProcessor::start(
                kv_persister,
                background_processor_invoice_payer.clone(),
                chain_monitor.clone(),
                background_processor_channel_manager.clone(),
                GossipSync::p2p(gossip_sync.clone()),
                background_processor_peer_manager.clone(),
                background_processor_logger.clone(),
                Some(scorer.clone()),
            );

            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                // Persistence errors here are non-fatal as we can just fetch the routing graph
                // again later, but they may indicate a disk error which could be fatal elsewhere.
                background_processor_logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!(
                        "background processor still running for node: {}",
                        background_processor_pubkey.clone()
                    ),
                    "node",
                    "",
                    0,
                ));
            }
        });

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
            invoice_payer,
            peer_manager,
            channel_manager,
            network_graph,
            onion_messenger,
            inbound_payments,
            outbound_payments,
        });
    }

    pub async fn connect_peer(
        &self,
        peer_pubkey_and_ip_addr: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if peer_pubkey_and_ip_addr == "" {
            self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: connectpeer requires peer connection info: `connectpeer pubkey@host:port`"),
                    "node",
                    "",
                    0,
                ));
            return Err("connectpeer requires peer connection info".into());
        };
        let (pubkey, peer_addr) = match parse_peer_info(peer_pubkey_and_ip_addr) {
            Ok(info) => info,
            Err(e) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: could not parse peer info: {}", e),
                    "node",
                    "",
                    0,
                ));
                return Err(e.into());
            }
        };
        if connect_peer_if_necessary(pubkey, peer_addr, self.peer_manager.clone())
            .await
            .is_ok()
        {
            self.logger.log(&Record::new(
                lightning::util::logger::Level::Info,
                format_args!("SUCCESS: connected to peer: {}", pubkey),
                "node",
                "",
                0,
            ));
        }

        Ok(())
    }

    pub fn create_wallet(&self) -> Result<(), Box<dyn std::error::Error>> {
        let create_wallet_bitcoind = self.ldk_bitcoind_client.clone();
        let create_wallet_pubkey = self.pubkey.clone();
        let create_wallet_logger = self.logger.clone();
        tokio::spawn(async move {
            match create_wallet_bitcoind.create_wallet(create_wallet_pubkey.clone()) {
                Ok(_) => {
                    create_wallet_logger.log(&Record::new(
                        lightning::util::logger::Level::Info,
                        format_args!("SUCCESS: created a wallet for this node"),
                        "node",
                        "",
                        0,
                    ));

                    match create_wallet_bitcoind.get_new_address(create_wallet_pubkey.clone()) {
                        Ok(addr) => {
                            create_wallet_logger.log(&Record::new(
                                lightning::util::logger::Level::Info,
                                format_args!("SUCCESS: created lightning node address: {}", addr),
                                "node",
                                "",
                                0,
                            ));
                        }
                        Err(e) => {
                            create_wallet_logger.log(&Record::new(
                                lightning::util::logger::Level::Error,
                                format_args!("ERROR: could not create node address: {}", e),
                                "node",
                                "",
                                0,
                            ));
                        }
                    }
                }
                Err(e) => {
                    create_wallet_logger.log(&Record::new(
                        lightning::util::logger::Level::Error,
                        format_args!("ERROR: could not create wallet for this node: {}", e),
                        "node",
                        "",
                        0,
                    ));
                }
            };
        });

        Ok(())
    }

    pub fn list_channels(&self) -> Vec<ChannelDetails> {
        self.channel_manager.list_channels()
    }

    pub fn list_peers(&self) -> Vec<String> {
        self.peer_manager
            .get_peer_node_ids()
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
    }

    pub async fn open_channel(
        &self,
        pubkey: String,
        amount_sat: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pubkey = to_compressed_pubkey(String::as_str(&pubkey.clone()));
        if pubkey.is_none() {
            self.logger.log(&Record::new(
                lightning::util::logger::Level::Error,
                format_args!("ERROR: could not parse peer pubkey"),
                "node",
                "",
                0,
            ));
            return Err("could not parse peer pubkey".into());
        }

        let config = UserConfig {
            channel_handshake_limits: ChannelHandshakeLimits {
                // lnd's max to_self_delay is 2016, so we want to be compatible.
                their_to_self_delay: 2016,
                ..Default::default()
            },
            channel_handshake_config: ChannelHandshakeConfig {
                announced_channel: true,
                ..Default::default()
            },
            ..Default::default()
        };

        match self
            .channel_manager
            .create_channel(pubkey.unwrap(), amount_sat, 0, 0, Some(config))
        {
            Ok(_) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("SUCCESS: channel initiated with peer: {:?}", pubkey),
                    "node",
                    "",
                    0,
                ));
                return Ok(());
            }
            Err(e) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: failed to open channel: {:?}", e),
                    "node",
                    "",
                    0,
                ));
                return Err("failed to open channel".into());
            }
        }
    }

    pub async fn close_channel(
        &self,
        channel_id: String,
        peer_pubkey: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let channel_id_vec = to_vec(String::as_str(&channel_id));
        if channel_id_vec.is_none() || channel_id_vec.as_ref().unwrap().len() != 32 {
            self.logger.log(&Record::new(
                lightning::util::logger::Level::Error,
                format_args!("ERROR: failed to parse channel_id"),
                "node",
                "",
                0,
            ));
            return Err("failed to open channel".into());
        }

        let mut channel_id = [0; 32];
        channel_id.copy_from_slice(&channel_id_vec.unwrap());

        let peer_pubkey_vec = match to_vec(String::as_str(&peer_pubkey)) {
            Some(peer_pubkey_vec) => peer_pubkey_vec,
            None => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: could not parse pubkey"),
                    "node",
                    "",
                    0,
                ));
                return Err("could not parse pubkey".into());
            }
        };
        let peer_pubkey = match PublicKey::from_slice(&peer_pubkey_vec) {
            Ok(peer_pubkey) => peer_pubkey,
            Err(_) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: could not parse pubkey"),
                    "node",
                    "",
                    0,
                ));
                return Err("could not parse pubkey".into());
            }
        };

        match self
            .channel_manager
            .close_channel(&channel_id, &peer_pubkey)
        {
            Ok(_) => Ok(()),
            Err(e) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: failed to close channel: {:?}", e),
                    "node",
                    "",
                    0,
                ));
                return Err("failed to open channel".into());
            }
        }
    }

    pub fn create_address(&self) -> Result<String, Box<dyn std::error::Error>> {
        match self
            .ldk_bitcoind_client
            .get_new_address(self.channel_manager.get_our_node_id().to_string())
        {
            Ok(res) => Ok(res.to_string()),
            Err(e) => Err(e),
        }
    }

    pub fn create_invoice(&self, amount_sat: u64) -> Result<String, Box<dyn std::error::Error>> {
        let mut payments = self.inbound_payments.lock().unwrap();
        let currency = Currency::Regtest;

        let invoice = match utils::create_invoice_from_channelmanager(
            &self.channel_manager,
            self.keys_manager.clone(),
            currency,
            Some(amount_sat * 1000),
            "lnsploit".to_string(),
            1500,
        ) {
            Ok(inv) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("SUCCESS: generated invoice: {}", inv),
                    "node",
                    "",
                    0,
                ));
                inv
            }
            Err(e) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: could not generate invoice: {}", e),
                    "node",
                    "",
                    0,
                ));
                return Err("could not generate invoice".into());
            }
        };

        let payment_hash = PaymentHash(invoice.payment_hash().clone().into_inner());
        payments.insert(
            payment_hash,
            PaymentInfo {
                preimage: None,
                secret: Some(invoice.payment_secret().clone()),
                status: HTLCStatus::Pending,
                amt_msat: MillisatAmount(Some(amount_sat * 1000)),
            },
        );
        Ok(invoice.to_string())
    }

    pub fn pay_invoice(&self, invoice_str: String) -> Result<(), Box<dyn std::error::Error>> {
        let invoice = match Invoice::from_str(&invoice_str) {
            Ok(inv) => inv,
            Err(e) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: invalid invoice: {}", e),
                    "node",
                    "",
                    0,
                ));
                return Err("invalid invoice".into());
            }
        };

        let status = match self.invoice_payer.pay_invoice(&invoice) {
            Ok(_payment_id) => {
                let payee_pubkey = invoice.recover_payee_pub_key();
                let amt_msat = invoice.amount_milli_satoshis().unwrap();
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!("SUCCESS: sending {} sats to: {}", amt_msat, payee_pubkey),
                    "node",
                    "",
                    0,
                ));
                HTLCStatus::Pending
            }
            Err(PaymentError::Invoice(e)) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: invalid invoice: {}", e),
                    "node",
                    "",
                    0,
                ));
                return Err("invalid invoice".into());
            }
            Err(PaymentError::Routing(e)) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: failed to find route: {:?}", e),
                    "node",
                    "",
                    0,
                ));
                return Err("failed to find route".into());
            }
            Err(PaymentError::Sending(e)) => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: failed to send payment: {:?}", e),
                    "node",
                    "",
                    0,
                ));
                HTLCStatus::Failed
            }
        };
        let payment_hash = PaymentHash(invoice.payment_hash().clone().into_inner());
        let payment_secret = Some(invoice.payment_secret().clone());

        let mut payments = self.outbound_payments.lock().unwrap();
        payments.insert(
            payment_hash,
            PaymentInfo {
                preimage: None,
                secret: payment_secret,
                status,
                amt_msat: MillisatAmount(invoice.amount_milli_satoshis()),
            },
        );

        Ok(())
    }
}

pub struct LdkEventHandler {
    channel_manager: Arc<RunnableChannelManager>,
    bitcoind_client: Arc<LdkBitcoindClient>,
    network_graph: Arc<NetworkGraph>,
    keys_manager: Arc<KeysManager>,
    inbound_payments: PaymentInfoStorage,
    outbound_payments: PaymentInfoStorage,
    network: Network,
    logger: Arc<FilesystemLogger>,
}

impl LdkEventHandler {
    fn new(
        channel_manager: Arc<RunnableChannelManager>,
        bitcoind_client: Arc<LdkBitcoindClient>,
        network_graph: Arc<NetworkGraph>,
        keys_manager: Arc<KeysManager>,
        inbound_payments: PaymentInfoStorage,
        outbound_payments: PaymentInfoStorage,
        network: Network,
        logger: Arc<FilesystemLogger>,
    ) -> Self {
        Self {
            channel_manager,
            bitcoind_client,
            network_graph,
            keys_manager,
            inbound_payments,
            outbound_payments,
            network,
            logger,
        }
    }
}

impl EventHandler for LdkEventHandler {
    fn handle_event(&self, event: &Event) {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                counterparty_node_id,
                channel_value_satoshis,
                output_script,
                ..
            } => {
                // Construct the raw transaction with one output, that is paid the amount of the
                // channel.
                let addr = WitnessProgram::from_scriptpubkey(
                    &output_script[..],
                    match self.network {
                        Network::Bitcoin => bitcoin_bech32::constants::Network::Bitcoin,
                        Network::Testnet => bitcoin_bech32::constants::Network::Testnet,
                        Network::Regtest => bitcoin_bech32::constants::Network::Regtest,
                        Network::Signet => bitcoin_bech32::constants::Network::Signet,
                    },
                )
                .expect("Lightning funding tx should always be to a SegWit output")
                .to_address();
                let mut outputs = HashMap::with_capacity(1);
                outputs.insert(addr, Amount::from_sat(*channel_value_satoshis));
                let raw_tx = self.bitcoind_client.create_raw_transaction(outputs);

                // Have your wallet put the inputs into the transaction such that the output is
                // satisfied.
                let funded_tx = self.bitcoind_client.fund_raw_transaction(raw_tx);

                // Sign the final funding transaction and broadcast it.
                let signed_tx = self
                    .bitcoind_client
                    .sign_raw_transaction_with_wallet(funded_tx.hex);
                assert_eq!(signed_tx.complete, true);
                let final_tx: Transaction =
                    encode::deserialize(&to_vec(&signed_tx.hex).unwrap()).unwrap();
                // Give the funding transaction back to LDK for opening the channel.
                if self
                    .channel_manager
                    .funding_transaction_generated(
                        &temporary_channel_id,
                        counterparty_node_id,
                        final_tx,
                    )
                    .is_err()
                {
                    self.logger.log(&Record::new(
                    lightning::util::logger::Level::Error,
                    format_args!("ERROR: Channel went away before we could fund it. The peer disconnected or refused the channel."),
                    "node",
                    "",
                    0,
                ));
                }
            }
            Event::PaymentReceived {
                payment_hash,
                purpose,
                amount_msat,
            } => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!(
                        "EVENT: received payment from payment hash {} of {} millisatoshis",
                        hex_str(&payment_hash.0),
                        amount_msat
                    ),
                    "node",
                    "",
                    0,
                ));

                let payment_preimage = match purpose {
                    PaymentPurpose::InvoicePayment {
                        payment_preimage, ..
                    } => *payment_preimage,
                    PaymentPurpose::SpontaneousPayment(preimage) => Some(*preimage),
                };
                self.channel_manager.claim_funds(payment_preimage.unwrap());
            }
            Event::PaymentClaimed {
                payment_hash,
                purpose,
                amount_msat,
            } => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!(
                        "EVENT: claimed payment from payment hash {} of {} millisatoshis",
                        hex_str(&payment_hash.0),
                        amount_msat
                    ),
                    "node",
                    "",
                    0,
                ));

                let (payment_preimage, payment_secret) = match purpose {
                    PaymentPurpose::InvoicePayment {
                        payment_preimage,
                        payment_secret,
                        ..
                    } => (*payment_preimage, Some(*payment_secret)),
                    PaymentPurpose::SpontaneousPayment(preimage) => (Some(*preimage), None),
                };
                let mut payments = self.inbound_payments.lock().unwrap();
                match payments.entry(*payment_hash) {
                    Entry::Occupied(mut e) => {
                        let payment = e.get_mut();
                        payment.status = HTLCStatus::Succeeded;
                        payment.preimage = payment_preimage;
                        payment.secret = payment_secret;
                    }
                    Entry::Vacant(e) => {
                        e.insert(PaymentInfo {
                            preimage: payment_preimage,
                            secret: payment_secret,
                            status: HTLCStatus::Succeeded,
                            amt_msat: MillisatAmount(Some(*amount_msat)),
                        });
                    }
                }
            }
            Event::PaymentSent {
                payment_preimage,
                payment_hash,
                fee_paid_msat,
                ..
            } => {
                let mut payments = self.outbound_payments.lock().unwrap();
                for (hash, payment) in payments.iter_mut() {
                    if *hash == *payment_hash {
                        payment.preimage = Some(*payment_preimage);
                        payment.status = HTLCStatus::Succeeded;

                        self.logger.log(&Record::new(
                        lightning::util::logger::Level::Info,
                        format_args!(
                            "EVENT: successfully sent payment of {} millisatoshis{} from payment hash {:?} with preimage {:?}",
                                payment.amt_msat,
                                if let Some(fee) = fee_paid_msat {
                                    format!(" (fee {} msat)", fee)
                                } else {
                                    "".to_string()
                                },
                                hex_str(&payment_hash.0),
                                hex_str(&payment_preimage.0)
                        ),
                        "node",
                        "",
                        0,
                    ));
                    }
                }
            }
            Event::OpenChannelRequest { .. } => {
                // Unreachable, we don't set manually_accept_inbound_channels
            }
            Event::PaymentPathSuccessful { .. } => {}
            Event::PaymentPathFailed { .. } => {}
            Event::ProbeSuccessful { .. } => {}
            Event::ProbeFailed { .. } => {}
            Event::PaymentFailed { payment_hash, .. } => {
                self.logger.log(&Record::new(
                lightning::util::logger::Level::Info,
                format_args!(
                    "EVENT: Failed to send payment to payment hash {:?}: exhausted payment retry attempts",
                    hex_str(&payment_hash.0)
                ),
                "node",
                "",
                0,
            ));

                let mut payments = self.outbound_payments.lock().unwrap();
                if payments.contains_key(&payment_hash) {
                    let payment = payments.get_mut(&payment_hash).unwrap();
                    payment.status = HTLCStatus::Failed;
                }
            }
            Event::PaymentForwarded {
                prev_channel_id,
                next_channel_id,
                fee_earned_msat,
                claim_from_onchain_tx,
            } => {
                let read_only_network_graph = self.network_graph.read_only();
                let nodes = read_only_network_graph.nodes();
                let channels = self.channel_manager.list_channels();

                let node_str = |channel_id: &Option<[u8; 32]>| match channel_id {
                    None => String::new(),
                    Some(channel_id) => match channels.iter().find(|c| c.channel_id == *channel_id)
                    {
                        None => String::new(),
                        Some(channel) => {
                            match nodes.get(&NodeId::from_pubkey(&channel.counterparty.node_id)) {
                                None => "private node".to_string(),
                                Some(node) => match &node.announcement_info {
                                    None => "unnamed node".to_string(),
                                    Some(announcement) => {
                                        format!("node {}", announcement.alias)
                                    }
                                },
                            }
                        }
                    },
                };
                let channel_str = |channel_id: &Option<[u8; 32]>| {
                    channel_id
                        .map(|channel_id| format!(" with channel {}", hex_str(&channel_id)))
                        .unwrap_or_default()
                };
                let from_prev_str = format!(
                    " from {}{}",
                    node_str(prev_channel_id),
                    channel_str(prev_channel_id)
                );
                let to_next_str = format!(
                    " to {}{}",
                    node_str(next_channel_id),
                    channel_str(next_channel_id)
                );

                let from_onchain_str = if *claim_from_onchain_tx {
                    "from onchain downstream claim"
                } else {
                    "from HTLC fulfill message"
                };
                if let Some(fee_earned) = fee_earned_msat {
                    self.logger.log(&Record::new(
                        lightning::util::logger::Level::Info,
                        format_args!(
                            "EVENT: Forwarded payment{}{}, earning {} msat {}",
                            from_prev_str, to_next_str, fee_earned, from_onchain_str
                        ),
                        "node",
                        "",
                        0,
                    ));
                } else {
                    self.logger.log(&Record::new(
                        lightning::util::logger::Level::Info,
                        format_args!(
                            "EVENT: Forwarded payment{}{}, claiming onchain {}",
                            from_prev_str, to_next_str, from_onchain_str
                        ),
                        "node",
                        "",
                        0,
                    ));
                }
            }
            Event::HTLCHandlingFailed { .. } => {}
            Event::PendingHTLCsForwardable { time_forwardable } => {
                let forwarding_channel_manager = self.channel_manager.clone();
                let min = time_forwardable.as_millis() as u64;
                tokio::spawn(async move {
                    let millis_to_sleep = rand::thread_rng().gen_range(min, min * 5) as u64;
                    tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
                    forwarding_channel_manager.process_pending_htlc_forwards();
                });
            }
            Event::SpendableOutputs { outputs } => {
                let destination_address = self
                    .bitcoind_client
                    .get_new_address(self.channel_manager.get_our_node_id().to_string())
                    .unwrap(); // TODO do not unwrap
                let output_descriptors = &outputs.iter().map(|a| a).collect::<Vec<_>>();
                let tx_feerate = self
                    .bitcoind_client
                    .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
                let spending_tx = self
                    .keys_manager
                    .spend_spendable_outputs(
                        output_descriptors,
                        Vec::new(),
                        destination_address.script_pubkey(),
                        tx_feerate,
                        &Secp256k1::new(),
                    )
                    .unwrap();
                self.bitcoind_client.broadcast_transaction(&spending_tx);
            }
            Event::ChannelClosed {
                channel_id,
                reason,
                user_channel_id: _,
            } => {
                self.logger.log(&Record::new(
                    lightning::util::logger::Level::Info,
                    format_args!(
                        "EVENT: Channel {} closed due to: {:?}",
                        hex_str(channel_id),
                        reason
                    ),
                    "node",
                    "",
                    0,
                ));
            }
            Event::DiscardFunding { .. } => {
                // A "real" node should probably "lock" the UTXOs spent in funding transactions until
                // the funding transaction either confirms, or this event is generated.
            }
        }
    }
}

pub(crate) type PaymentInfoStorage = Arc<std::sync::Mutex<HashMap<PaymentHash, PaymentInfo>>>;

pub(crate) struct PaymentInfo {
    preimage: Option<PaymentPreimage>,
    secret: Option<PaymentSecret>,
    status: HTLCStatus,
    amt_msat: MillisatAmount,
}

pub(crate) type InvoicePayer<E> = payment::InvoicePayer<
    Arc<RunnableChannelManager>,
    Router,
    Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>, Arc<FilesystemLogger>>>>,
    Arc<FilesystemLogger>,
    E,
>;

type Router = DefaultRouter<Arc<NetworkGraph>, Arc<FilesystemLogger>>;

pub(crate) enum HTLCStatus {
    Pending,
    Succeeded,
    Failed,
}

pub(crate) struct MillisatAmount(Option<u64>);

impl fmt::Display for MillisatAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Some(amt) => write!(f, "{}", amt),
            None => write!(f, "unknown"),
        }
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

pub fn to_vec(hex: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(hex.len() / 2);

    let mut b = 0;
    for (idx, c) in hex.as_bytes().iter().enumerate() {
        b <<= 4;
        match *c {
            b'A'..=b'F' => b |= c - b'A' + 10,
            b'a'..=b'f' => b |= c - b'a' + 10,
            b'0'..=b'9' => b |= c - b'0',
            _ => return None,
        }
        if (idx & 1) == 1 {
            out.push(b);
            b = 0;
        }
    }

    Some(out)
}

#[inline]
pub fn hex_str(value: &[u8]) -> String {
    let mut res = String::with_capacity(64);
    for v in value {
        res += &format!("{:02x}", v);
    }
    res
}

pub fn to_compressed_pubkey(hex: &str) -> Option<PublicKey> {
    let data = match to_vec(&hex[0..33 * 2]) {
        Some(bytes) => bytes,
        None => return None,
    };
    match PublicKey::from_slice(&data) {
        Ok(pk) => Some(pk),
        Err(_) => None,
    }
}

pub(crate) fn parse_peer_info(
    peer_pubkey_and_ip_addr: String,
) -> Result<(PublicKey, SocketAddr), std::io::Error> {
    let mut pubkey_and_addr = peer_pubkey_and_ip_addr.split("@");
    let pubkey = pubkey_and_addr.next();
    let peer_addr_str = pubkey_and_addr.next();
    if peer_addr_str.is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "ERROR: incorrectly formatted peer info. Should be formatted as: `pubkey@host:port`",
        ));
    }

    let peer_addr = peer_addr_str
        .unwrap()
        .to_socket_addrs()
        .map(|mut r| r.next());
    if peer_addr.is_err() || peer_addr.as_ref().unwrap().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "ERROR: couldn't parse pubkey@host:port into a socket address",
        ));
    }

    let pubkey = to_compressed_pubkey(pubkey.unwrap());
    if pubkey.is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "ERROR: unable to parse given pubkey for node",
        ));
    }

    Ok((pubkey.unwrap(), peer_addr.unwrap().unwrap()))
}

pub(crate) async fn connect_peer_if_necessary(
    pubkey: PublicKey,
    peer_addr: SocketAddr,
    peer_manager: Arc<PeerManager>,
) -> Result<(), ()> {
    for node_pubkey in peer_manager.get_peer_node_ids() {
        if node_pubkey == pubkey {
            return Ok(());
        }
    }
    let res = do_connect_peer(pubkey, peer_addr, peer_manager).await;
    res
}

pub(crate) async fn do_connect_peer(
    pubkey: PublicKey,
    peer_addr: SocketAddr,
    peer_manager: Arc<PeerManager>,
) -> Result<(), ()> {
    match lightning_net_tokio::connect_outbound(Arc::clone(&peer_manager), pubkey, peer_addr).await
    {
        Some(connection_closed_future) => {
            let mut connection_closed_future = Box::pin(connection_closed_future);
            loop {
                match futures::poll!(&mut connection_closed_future) {
                    std::task::Poll::Ready(_) => {
                        return Err(());
                    }
                    std::task::Poll::Pending => {}
                }
                // Avoid blocking the tokio context by sleeping a bit
                match peer_manager
                    .get_peer_node_ids()
                    .iter()
                    .find(|peer_node_id| **peer_node_id == pubkey)
                {
                    Some(_) => return Ok(()),
                    None => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        }
        None => Err(()),
    }
}
