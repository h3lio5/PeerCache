# PeerCache
PeerCache is a distributed key-value data store built using Rust and leveraging libp2p and tokio
libraries. Used the FloodSub protocol for efficient retrieval of stored messages across a decentralized network
of nodes. The code is written to store "NFT" metadata with predefined fields (I wanted to get the implementation running quickly, so I made the data type concrete) but can be easily adapted to store any kind of data with minor modification to the code. 

Note: The implementation may look hacky because I only intended to write this code for learning how to work with the rust-libp2p library and not to make a full fledged high performant distributed key value store. 


#TODO: Update README
## Instructions
1. Install Rust v1.61.0-nightly (follow instructions [here](https://doc.rust-lang.org/book/ch01-01-installation.html)).  
2. Clone the repo: `git clone https://github.com/h3lio5/rusty-messenger.git`  
3. Start the p2p network by running the command `cargo run RUST_LOG=info cargo run` in multiple terminal tabs.     
4. Interact with the network using the following commands (run the commands in different terminal tabs)-
* LIST PEERS: Lists all the peers connected to your node
* CREATE NFT <collection_name>|<item_id>|<description>|<owner> (NOTE: all the fields are necessary)
* GET NFT ALL: lists all the NFTs stored on the network
* GET NFT <collection_name>: Lists all the NFTs of the requested collection
