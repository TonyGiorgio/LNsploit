use super::schema::channel_managers::dsl::*;
use super::schema::channel_updates::dsl::*;
use super::schema::{channel_managers, channel_updates};
use super::RunnableChannelManager;
use bitcoin::hashes::hex::ToHex;
use diesel::{prelude::*, r2d2::ConnectionManager, r2d2::Pool};

use lightning::chain::chainmonitor;
use lightning::chain::channelmonitor::{ChannelMonitor, ChannelMonitorUpdate};
use lightning::chain::keysinterface::Sign;
use lightning::chain::transaction::OutPoint;
use lightning::chain::ChannelMonitorUpdateErr;
use lightning::util::ser::{Writeable, Writer};
use std::io::Error;
use std::io::{Cursor, Read, Seek, SeekFrom};
use uuid::Uuid;

#[derive(Queryable)]
pub struct ChannelManager {
    pub id: String,
    pub node_id: String,
    pub channel_tx_id: String,
    pub channel_tx_index: i32,
    pub channel_monitor_data: Vec<u8>,
}

#[derive(Insertable)]
#[diesel(table_name = channel_managers)]
pub struct NewChannelManager<'a> {
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
}

impl<ChannelSigner: Sign> chainmonitor::Persist<ChannelSigner> for NodePersister {
    fn persist_new_channel(
        &self,
        funding_txo: OutPoint,
        monitor: &ChannelMonitor<ChannelSigner>,
        _update_id: chainmonitor::MonitorUpdateId,
    ) -> Result<(), ChannelMonitorUpdateErr> {
        // save channel to SQL table
        // node_db_id, funding txid, funding index, monitor data

        let monitor_data = write_to_memory(monitor);
        let funding_txo_txid = funding_txo.txid.to_hex();

        // First detect if it already exists, then update it
        // if it does not exist then add it
        let conn = &mut self.db.get().unwrap();
        let channel_manager_list = channel_managers
            .filter(super::schema::channel_managers::node_id.eq(self.node_db_id.clone()))
            .filter(super::schema::channel_managers::channel_tx_id.eq(funding_txo_txid.clone()))
            .filter(super::schema::channel_managers::channel_tx_index.eq(funding_txo.index as i32))
            .load::<ChannelManager>(conn)
            .expect("error loading channel managers");
        match channel_manager_list.len() {
            0 => {
                // no channel manager for this node & outpoint, create
                let new_channel_manager_id = Uuid::new_v4().to_string();
                let new_channel_manager = NewChannelManager {
                    id: String::as_str(&new_channel_manager_id),
                    node_id: String::as_str(&self.node_db_id),
                    channel_tx_id: String::as_str(&funding_txo_txid),
                    channel_tx_index: funding_txo.index as i32,
                    channel_monitor_data: monitor_data,
                };
                match diesel::insert_into(channel_managers)
                    .values(&new_channel_manager)
                    .execute(conn)
                {
                    Ok(_) => (),
                    Err(_) => return Err(ChannelMonitorUpdateErr::PermanentFailure),
                }
            }
            1 => {
                // a channel manager already exists, overwrite
                match diesel::update(channel_managers)
                    .filter(super::schema::channel_managers::node_id.eq(self.node_db_id.clone()))
                    .filter(
                        super::schema::channel_managers::channel_tx_id.eq(funding_txo_txid.clone()),
                    )
                    .filter(
                        super::schema::channel_managers::channel_tx_index
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
        monitor: &ChannelMonitor<ChannelSigner>,
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
                // save the entire manager for block related updates
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

impl<Signer: Sign> DiskWriteable for ChannelMonitor<Signer> {
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
