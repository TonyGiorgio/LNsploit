# LNsploit

A Lightning Network exploit toolkit.

## Future/Feature Ideas

- UI:

  - Based on [tui-rs](https://github.com/fdehau/tui-rs)
  - A network explorer view of some sort
    - 20k+ nodes is a lot.. maybe do some filtering such as large value channels only?
    - Color code nodes that are owned by you, maybe have a 6 hop view of nodes you can reach easily
  - A list of nodes you own, since in LDK there can be many
  - Node management screen
    - Start
    - Stop
    - Create New
    - Open Channel
    - Close Channel
    - Make Payment
  - An active list of successful / failed payments you are routing
  - Payments you have made
  - Searchable/paged list of nodes on the network
    - Selecting a node gives you actions you can make on it
      - Connecting
      - Opening channel
      - Exploit
  - A list of usable exploits to use
    - Selecting an exploit takes you to a page where you select the node to target.

- Possible Exploits
  - LND <v0.15.2 script [bug #7002](https://github.com/lightningnetwork/lnd/issues/7002) ✅
  - Broadcast old revocation commitment
  - Balance probe
  - Channel Jam
  - Probe for unannounced channels
  - Known CVE's on old versions of implementations

## Development

### Running

Copy the default config provided in this repo.

```
cp config.yaml local.config.yaml
```

Change the values if necessary and then run with those configs.

```
cargo run local.config.yaml
```

### Database Migrations

Changing any migration data will require running the migration locally on the development machine so that rust schema.rs code can also be generated:

```
DATABASE_URL=database.db diesel migration run
```

To create a new migration file, do this:

```
diesel migration generate node_channels
```
