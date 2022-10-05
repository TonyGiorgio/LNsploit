CREATE TABLE key_values (
  id VARCHAR PRIMARY KEY NOT NULL,
  node_id CHAR(36) NOT NULL REFERENCES nodes(id),
  data_value BLOB NOT NULL
);
