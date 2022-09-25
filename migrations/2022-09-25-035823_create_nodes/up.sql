CREATE TABLE nodes (
  id CHAR(36) PRIMARY KEY NOT NULL,
  pubkey VARCHAR NOT NULL
);

CREATE TABLE master_keys (
  id CHAR(36) PRIMARY KEY NOT NULL,
  seed BLOB NOT NULL,
  mnemonic VARCHAR NOT NULL
);

CREATE TABLE node_keys (
  id CHAR(36) PRIMARY KEY NOT NULL,
  key_id CHAR(36) NOT NULL REFERENCES master_keys(id),
  node_id CHAR(36) NOT NULL REFERENCES nodes(id),
  UNIQUE(node_id)
);
