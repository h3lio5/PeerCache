# Rusty Messenger
A toy peer-to-peer application that allows users to upload NFT details on the network, search for all the NFTs present on the network, search for specific NFT collection name through messaging the network peers.

#TODO: Update README
## Instructions
1. Install Rust v1.61.0-nightly (follow instructions [here](https://doc.rust-lang.org/book/ch01-01-installation.html)).  
2. Clone the repo: `git clone https://github.com/h3lio5/rusty-messenger.git`  
3. Start the p2p network by running the command `cargo run RUST_LOG=info cargo run` in multiple terminal tabs.     
4. Interact with the network using the following commands-
* LIST PEERS: Lists all the peers connected to your node
* CREATE NFT <collection_name>|<item_id>|<description>|<owner> (NOTE: all the fields are necessary)
* GET NFT ALL: lists all the NFTs stored on the network
* GET NFT <collection_name>: Lists all the NFTs of the requested collection
