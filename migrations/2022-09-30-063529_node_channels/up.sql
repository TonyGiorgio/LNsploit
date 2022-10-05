CREATE TABLE channel_monitors (
  id CHAR(36) PRIMARY KEY NOT NULL,
  node_id CHAR(36) NOT NULL REFERENCES nodes(id),
  channel_tx_id CHAR(64) NOT NULL,
  channel_tx_index INTEGER NOT NULL,
  channel_monitor_data BLOB NOT NULL,
  UNIQUE(node_id, channel_tx_id, channel_tx_index)
);

CREATE TABLE channel_updates (
  id CHAR(36) PRIMARY KEY NOT NULL,
  node_id CHAR(36) NOT NULL REFERENCES nodes(id),
  channel_tx_id CHAR(64) NOT NULL,
  channel_tx_index INTEGER NOT NULL,
  channel_internal_update_id INTEGER NOT NULL,
  channel_update_data BLOB NOT NULL,
  UNIQUE(node_id, channel_tx_id, channel_tx_index, channel_internal_update_id)
);
