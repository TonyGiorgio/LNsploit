# LNsploit

A Lightning Network exploit toolkit.

## Feature Ideas

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


- Exploits
  - Balance probe
  - Channel Jam
  - Probe for unannounced channels
  - Known CVE's on old versions of implementations
