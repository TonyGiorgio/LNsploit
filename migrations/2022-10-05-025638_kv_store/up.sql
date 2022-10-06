CREATE TABLE key_values (
  key_value_id CHAR(36) PRIMARY KEY NOT NULL,
  id VARCHAR NOT NULL,
  node_id CHAR(36) NOT NULL,
  data_value BLOB NOT NULL,
  UNIQUE(id, node_id)
);
