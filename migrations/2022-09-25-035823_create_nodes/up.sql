CREATE TABLE master_keys (
  id CHAR(36) PRIMARY KEY NOT NULL,
  seed BLOB NOT NULL,
  mnemonic VARCHAR NOT NULL
);

CREATE TABLE node_keys (
  id CHAR(36) PRIMARY KEY NOT NULL,
  master_key_id CHAR(36) NOT NULL REFERENCES master_keys(id),
  child_index INTEGER NOT NULL,
  UNIQUE(child_index)
);

CREATE TABLE nodes (
  id CHAR(36) PRIMARY KEY NOT NULL,
  pubkey VARCHAR NOT NULL,
  key_id CHAR(36) NOT NULL REFERENCES node_keys(id)
);

