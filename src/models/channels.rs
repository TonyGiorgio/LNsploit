use super::schema::channel_monitors::dsl::*;
use super::schema::channel_updates::dsl::*;
use super::schema::{channel_monitors, channel_updates};
use super::RunnableChannelManager;
use bitcoin::hashes::hex::{FromHex, ToHex};
use diesel::{prelude::*, r2d2::ConnectionManager, r2d2::Pool};

use bitcoin::hash_types::{BlockHash, Txid};
use lightning::chain::chainmonitor;
use lightning::chain::channelmonitor;
use lightning::chain::channelmonitor::ChannelMonitorUpdate;
use lightning::chain::keysinterface::{KeysInterface, Sign};
use lightning::chain::transaction::OutPoint;
use lightning::chain::ChannelMonitorUpdateErr;
use lightning::util::ser::{Readable, ReadableArgs, Writeable, Writer};
use std::collections::HashMap;
use std::io::Error;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Queryable)]
pub struct ChannelMonitor {
    pub id: String,
    pub node_id: String,
    pub channel_tx_id: String,
    pub channel_tx_index: i32,
    pub channel_monitor_data: Vec<u8>,
}

#[derive(Insertable)]
#[diesel(table_name = channel_monitors)]
pub struct NewChannelMonitor<'a> {
    pub id: &'a str,
    pub node_id: &'a str,
    pub channel_tx_id: &'a str,
    pub channel_tx_index: i32,
    pub channel_monitor_data: Vec<u8>,
}

#[derive(Queryable)]
pub struct ChannelUpdate {
    pub id: String,
    pub node_id: String,
    pub channel_tx_id: String,
    pub channel_tx_index: i32,
    pub channel_internal_update_id: i32,
    pub channel_update_data: Vec<u8>,
}

#[derive(Insertable)]
#[diesel(table_name = channel_updates)]
pub struct NewChannelUpdate<'a> {
    pub id: &'a str,
    pub node_id: &'a str,
    pub channel_tx_id: &'a str,
    pub channel_tx_index: i32,
    pub channel_internal_update_id: i32,
    pub channel_update_data: Vec<u8>,
}

pub struct NodePersister {
    db: Pool<ConnectionManager<SqliteConnection>>,
    pub node_db_id: String,
}

impl NodePersister {
    pub fn new(db: Pool<ConnectionManager<SqliteConnection>>, node_db_id: String) -> Self {
        return Self { db, node_db_id };
    }

    pub fn read_channelmonitors<Signer: Sign, K: Deref>(
        &self,
        keys_manager: K,
    ) -> Result<Vec<(BlockHash, channelmonitor::ChannelMonitor<Signer>)>, std::io::Error>
    where
        K::Target: KeysInterface<Signer = Signer> + Sized,
    {
        let conn = &mut self.db.get().unwrap();
        let mut res = Vec::new();

        // Get all the channel monitor buffers that exist for this node
        let channel_monitor_list = channel_monitors
            .filter(super::schema::channel_monitors::node_id.eq(self.node_db_id.clone()))
            .load::<ChannelMonitor>(conn)
            .expect("error loading channel monitors");

        for channel_monitor_item in channel_monitor_list {
            let txid = Txid::from_hex(String::as_str(&channel_monitor_item.channel_tx_id));
            if txid.is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid tx ID in db",
                ));
            }
            let index = channel_monitor_item.channel_tx_index;

            let contents = channel_monitor_item.channel_monitor_data;
            let mut buffer = Cursor::new(&contents);
            match <(BlockHash, channelmonitor::ChannelMonitor<Signer>)>::read(
                &mut buffer,
                &*keys_manager,
            ) {
                Ok((blockhash, channel_monitor)) => {
                    if channel_monitor.get_funding_txo().0.txid != txid.unwrap()
                        || channel_monitor.get_funding_txo().0.index != index as u16
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "ChannelMonitor was stored in the wrong file",
                        ));
                    }
                    res.push((blockhash, channel_monitor));
                }
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize ChannelMonitor: {}", e),
                    ))
                }
            }
        }

        Ok(res)
    }

    pub fn read_channelmonitor_updates(
        &self,
    ) -> Result<HashMap<Txid, Vec<ChannelMonitorUpdate>>, std::io::Error> {
        let mut tx_id_channel_map: HashMap<Txid, Vec<ChannelMonitorUpdate>> = HashMap::new();
        let conn = &mut self.db.get().unwrap();

        let channel_monitor_update_list = channel_updates
            .filter(super::schema::channel_updates::node_id.eq(self.node_db_id.clone()))
            .load::<ChannelUpdate>(conn)
            .expect("error loading channel monitors");

        for channel_update_item in channel_monitor_update_list {
            let txid = Txid::from_hex(String::as_str(&channel_update_item.channel_tx_id));
            if txid.is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid tx ID in db",
                ));
            }
            // let index = channel_update_item.channel_tx_index;

            let contents = channel_update_item.channel_update_data;
            let mut buffer = Cursor::new(&contents);
            match <ChannelMonitorUpdate>::read(&mut buffer) {
                Ok(channel_monitor_update) => {
                    // see if we already have this key
                    match tx_id_channel_map.get_mut(&txid.unwrap()) {
                        Some(map) => map.push(channel_monitor_update),
                        None => {
                            tx_id_channel_map.insert(txid.unwrap(), vec![channel_monitor_update]);
                        }
                    }
                }
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize ChannelMonitorUpdate: {}", e),
                    ))
                }
            }
        }

        Ok(tx_id_channel_map)
    }
}

impl<ChannelSigner: Sign> chainmonitor::Persist<ChannelSigner> for NodePersister {
    fn persist_new_channel(
        &self,
        funding_txo: OutPoint,
        monitor: &channelmonitor::ChannelMonitor<ChannelSigner>,
        _update_id: chainmonitor::MonitorUpdateId,
    ) -> Result<(), ChannelMonitorUpdateErr> {
        // save channel to SQL table
        // node_db_id, funding txid, funding index, monitor data

        let monitor_data = write_to_memory(monitor);
        let funding_txo_txid = funding_txo.txid.to_hex();

        // First detect if it already exists, then update it
        // if it does not exist then add it
        let conn = &mut self.db.get().unwrap();
        let channel_monitor_list = channel_monitors
            .filter(super::schema::channel_monitors::node_id.eq(self.node_db_id.clone()))
            .filter(super::schema::channel_monitors::channel_tx_id.eq(funding_txo_txid.clone()))
            .filter(super::schema::channel_monitors::channel_tx_index.eq(funding_txo.index as i32))
            .load::<ChannelMonitor>(conn)
            .expect("error loading channel monitors");
        match channel_monitor_list.len() {
            0 => {
                // no channel monitor for this node & outpoint, create
                let new_channel_monitor_id = Uuid::new_v4().to_string();
                let new_channel_monitor = NewChannelMonitor {
                    id: String::as_str(&new_channel_monitor_id),
                    node_id: String::as_str(&self.node_db_id),
                    channel_tx_id: String::as_str(&funding_txo_txid),
                    channel_tx_index: funding_txo.index as i32,
                    channel_monitor_data: monitor_data,
                };
                match diesel::insert_into(channel_monitors)
                    .values(&new_channel_monitor)
                    .execute(conn)
                {
                    Ok(_) => (),
                    Err(_) => return Err(ChannelMonitorUpdateErr::PermanentFailure),
                }
            }
            1 => {
                // a channel monitor already exists, overwrite
                match diesel::update(channel_monitors)
                    .filter(super::schema::channel_monitors::node_id.eq(self.node_db_id.clone()))
                    .filter(
                        super::schema::channel_monitors::channel_tx_id.eq(funding_txo_txid.clone()),
                    )
                    .filter(
                        super::schema::channel_monitors::channel_tx_index
                            .eq(funding_txo.index as i32),
                    )
                    .set(channel_monitor_data.eq(monitor_data))
                    .execute(conn)
                {
                    Ok(_) => (),
                    Err(_) => return Err(ChannelMonitorUpdateErr::PermanentFailure),
                }
            }
            _ => return Err(ChannelMonitorUpdateErr::PermanentFailure),
        };

        // anytime monitor data is written, delete the temp update data
        match diesel::delete(
            channel_updates
                .filter(super::schema::channel_updates::node_id.eq(self.node_db_id.clone()))
                .filter(super::schema::channel_updates::channel_tx_id.eq(funding_txo_txid.clone()))
                .filter(
                    super::schema::channel_updates::channel_tx_index.eq(funding_txo.index as i32),
                ),
        )
        .execute(conn)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(ChannelMonitorUpdateErr::PermanentFailure),
        }
    }

    fn update_persisted_channel(
        &self,
        funding_txo: OutPoint,
        update: &Option<ChannelMonitorUpdate>,
        monitor: &channelmonitor::ChannelMonitor<ChannelSigner>,
        update_id: chainmonitor::MonitorUpdateId,
    ) -> Result<(), ChannelMonitorUpdateErr> {
        match update.is_some() {
            true => {
                // save just the update into its own table
                // node_db_id, txid, index, update_id, update data
                let conn = &mut self.db.get().unwrap();

                let monitor_data = write_to_memory(monitor);
                let funding_txo_txid = funding_txo.txid.to_hex();

                let new_channel_update_id = Uuid::new_v4().to_string();
                let new_channel_update = NewChannelUpdate {
                    id: String::as_str(&new_channel_update_id),
                    node_id: String::as_str(&self.node_db_id),
                    channel_tx_id: String::as_str(&funding_txo_txid),
                    channel_tx_index: funding_txo.index as i32,
                    channel_internal_update_id: update.clone().unwrap().update_id as i32,
                    channel_update_data: monitor_data,
                };
                match diesel::insert_into(channel_updates)
                    .values(&new_channel_update)
                    .execute(conn)
                {
                    Ok(_) => Ok(()),
                    Err(_) => Err(ChannelMonitorUpdateErr::PermanentFailure),
                }
            }
            false => {
                // save the entire monitor for block related updates
                // this behaves exactly the same as persisting a new channel
                return self.persist_new_channel(funding_txo, monitor, update_id);
            }
        }
    }
}

pub(crate) trait DiskWriteable {
    fn write_to_memory<W: Writer>(&self, writer: &mut W) -> Result<(), std::io::Error>;
}

impl DiskWriteable for RunnableChannelManager {
    fn write_to_memory<W: Writer>(&self, writer: &mut W) -> Result<(), Error> {
        self.write(writer)
    }
}

impl<Signer: Sign> DiskWriteable for channelmonitor::ChannelMonitor<Signer> {
    fn write_to_memory<W: Writer>(&self, writer: &mut W) -> Result<(), Error> {
        self.write(writer)
    }
}

impl DiskWriteable for ChannelMonitorUpdate {
    fn write_to_memory<W: Writer>(&self, writer: &mut W) -> Result<(), Error> {
        self.write(writer)
    }
}

#[allow(bare_trait_objects)]
pub(crate) fn write_to_memory<D: DiskWriteable>(data: &D) -> Vec<u8> {
    let mut monitor_data_cursor = Cursor::new(Vec::new());
    data.write_to_memory(&mut monitor_data_cursor).unwrap();
    monitor_data_cursor.seek(SeekFrom::Start(0)).unwrap();
    let mut monitor_data = Vec::new();
    monitor_data_cursor.read_to_end(&mut monitor_data).unwrap();
    monitor_data
}
