use super::schema::channel_managers::dsl::*;
use super::schema::channel_updates::dsl::*;
use super::schema::{channel_managers, channel_updates};
use diesel::{prelude::*, r2d2::ConnectionManager, r2d2::Pool};

use lightning::chain::chainmonitor;
use lightning::chain::channelmonitor::{ChannelMonitor, ChannelMonitorUpdate};
use lightning::chain::keysinterface::Sign;
use lightning::chain::transaction::OutPoint;
use lightning::chain::ChannelMonitorUpdateErr;

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

        // anytime monitor data is written, delete the temp update data

        /*
        let filename = format!("{}_{}", funding_txo.txid.to_hex(), funding_txo.index);
        let write_res = write_to_file(self.path_to_monitor_data(), filename, monitor)
            .map_err(|_| chain::ChannelMonitorUpdateErr::PermanentFailure);
        if write_res.is_err() {
            return write_res;
        }
        // anytime monitor data is written, delete the update dir
        fs::create_dir_all(self.path_to_monitor_data_updates().clone()).unwrap();
        fs::remove_dir_all(self.path_to_monitor_data_updates()).unwrap();
        fs::create_dir(self.path_to_monitor_data_updates()).unwrap();
        */
        Ok(())
    }

    fn update_persisted_channel(
        &self,
        outpoint_id: OutPoint,
        update: &Option<ChannelMonitorUpdate>,
        data: &ChannelMonitor<ChannelSigner>,
        _update_id: chainmonitor::MonitorUpdateId,
    ) -> Result<(), ChannelMonitorUpdateErr> {
        if update.is_some() {
            // save just the update into its own table
            // node_db_id, txid, index, update_id, update data

            /*
            fs::create_dir_all(self.path_to_monitor_data_updates().clone()).unwrap();
            let filename = format!(
                "{}_{}_{}",
                id.txid.to_hex(),
                id.index,
                update.clone().unwrap().update_id
            );
            write_to_file(
                self.path_to_monitor_data_updates(),
                filename,
                &update.clone().unwrap(),
            )
            .map_err(|_| chain::ChannelMonitorUpdateErr::PermanentFailure)
            .unwrap();
            */
        } else {
            // save the entire manager for block related updates
            //
            // after the entire manager is saved, drop update rows associated with it, not needed
            // anymore
            /*
            let filename = format!("{}_{}", id.txid.to_hex(), id.index);
            write_to_file(self.path_to_monitor_data(), filename, data)
                .map_err(|_| chain::ChannelMonitorUpdateErr::PermanentFailure)
                .unwrap();

            // then delete the updates file since manager includes them
            self.chan_update_cache.write().unwrap().remove(&id);

            // also delete the update dir

            fs::create_dir_all(self.path_to_monitor_data_updates().clone()).unwrap();
            fs::remove_dir_all(self.path_to_monitor_data_updates()).unwrap();
            fs::create_dir(self.path_to_monitor_data_updates()).unwrap();
            */
        }
        Ok(())
    }
}
