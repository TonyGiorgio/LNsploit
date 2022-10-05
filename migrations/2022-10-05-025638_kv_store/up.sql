CREATE TABLE key_values (
  id VARCHAR NOT NULL,
  node_id CHAR(36) NOT NULL REFERENCES nodes(id),
  data_value BLOB NOT NULL,
  UNIQUE(id, node_id)
);
