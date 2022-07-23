use futures::task;
use libp2p::{
    core::upgrade,
    floodsub::{Floodsub, FloodsubEvent, Topic},
    futures::StreamExt,
    identity,
    kad::{store::MemoryStore, Kademlia},
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder, SwarmEvent},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Transport,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::{fs, io::AsyncBufReadExt, sync::mpsc};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;
type NFTInfoList = Vec<NFTInfo>;

static KEYS: Lazy<identity::Keypair> = Lazy::new(|| identity::Keypair::generate_ed25519());
static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("nft_info"));
static mut NFT_STORE: NFTInfoList = Vec::new();

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NFTInfo {
    collection_name: String,
    item_id: u32,
    description: String,
    owner: String,
}

#[derive(Debug, Serialize, Deserialize)]
enum ListMode {
    ALL,
    Collection(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct ListRequest {
    mode: ListMode,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListResponse {
    mode: ListMode,
    data: NFTInfoList,
    receiver: String,
}

enum EventType {
    Response(ListResponse),
    Input(String),
}

#[derive(NetworkBehaviour)]
struct NFTInfoBehaviour {
    floodsub: Floodsub,
    mdns: Mdns,
    #[behaviour(ignore)]
    response_sender: mpsc::UnboundedSender<ListResponse>,
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for NFTInfoBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        match event {
            FloodsubEvent::Message(msg) => {
                if let Ok(resp) = serde_json::from_slice::<ListResponse>(&msg.data) {
                    if resp.receiver == PEER_ID.to_string() {
                        info!("Response from {}:", msg.source);
                        resp.data.iter().for_each(|r| info!("{:?}", r));
                    }
                } else if let Ok(ref req) = serde_json::from_slice::<ListRequest>(&msg.data) {
                    match &req.mode {
                        ListMode::ALL => {
                            info!("Received ALL req: {:?} from {:?}", req, msg.source);
                            respond_with_all_nft_info(
                                self.response_sender.clone(),
                                msg.source.to_string(),
                            );
                        }
                        ListMode::Collection(collection_name) => {
                            info!("Received collection req: {:?} from {:?}", req, msg.source);
                            respond_with_collection_nft_info(
                                self.response_sender.clone(),
                                msg.source.to_string(),
                                collection_name.clone(),
                            );
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

fn respond_with_collection_nft_info(
    sender: mpsc::UnboundedSender<ListResponse>,
    receiver: String,
    collection_name: String,
) {
    tokio::spawn(async move {
        let nft_info = read_local_nft_info().clone();
        let resp_data = nft_info
            .into_iter()
            .filter(|r| r.collection_name.eq_ignore_ascii_case(&collection_name))
            .collect::<Vec<_>>();
        // If only the peer has any collection items, send them back to the message origin
        if resp_data.len() > 0 {
            let response = ListResponse {
                mode: ListMode::Collection(collection_name),
                receiver,
                data: resp_data,
            };
            if let Err(e) = sender.send(response) {
                error!("error sending response via channel, {}", e);
            }
        }
    });
}

fn respond_with_all_nft_info(sender: mpsc::UnboundedSender<ListResponse>, receiver: String) {
    tokio::spawn(async move {
        let nft_info = read_local_nft_info().clone();
        let resp = ListResponse {
            mode: ListMode::ALL,
            receiver,
            data: nft_info,
        };
        if let Err(e) = sender.send(resp) {
            error!("error sending response via channel, {}", e);
        }
    });
}

impl NetworkBehaviourEventProcess<MdnsEvent> for NFTInfoBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

async fn create_new_nft_info(
    collection_name: &str,
    item_id: u32,
    description: &str,
    owner: &str,
) -> Result<()> {
    let mut local_nft_info = read_local_nft_info();

    local_nft_info.push(NFTInfo {
        collection_name: collection_name.to_owned(),
        item_id: item_id.clone(),
        description: description.to_owned(),
        owner: owner.to_owned(),
    });

    info!("Created NFT info:");
    info!("Name: {}", collection_name);
    info!("Item ID: {}", item_id);
    info!("NFT Item Description  {}", description);
    info!("NFT Item owner {}", owner);

    Ok(())
}

fn read_local_nft_info() -> &'static mut NFTInfoList {
    // DANGEROUS; have to come up with a better solution.
    return unsafe { &mut NFT_STORE };
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    info!("Peer Id: {}", PEER_ID.clone());
    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();

    let auth_keys = Keypair::<X25519Spec>::new()
        .into_authentic(&KEYS)
        .expect("can create auth keys");

    let transp = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let mut behaviour = NFTInfoBehaviour {
        floodsub: Floodsub::new(PEER_ID.clone()),
        mdns: Mdns::new(MdnsConfig::default())
            .await
            .expect("can create mdns"),
        response_sender,
    };

    behaviour.floodsub.subscribe(TOPIC.clone());

    let mut swarm = SwarmBuilder::new(transp, behaviour, PEER_ID.clone())
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build();

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("can get a local socket"),
    )
    .expect("swarm can be started");

    loop {
        let evt = {
            tokio::select! {
                line = stdin.next_line() => Some(EventType::Input(line.expect("can get line").expect("can read line from stdin"))),
                response = response_rcv.recv() => Some(EventType::Response(response.expect("response exists"))),
                // Commenting out the below event logs as it was creating a lot of clutter on the terminal screen
                event = swarm.select_next_some() => match event {
                    // SwarmEvent::NewListenAddr { address, .. } => {
                    //     println!("Listening in {:?}", address);
                    //     None
                    // },
                    // SwarmEvent::ConnectionEstablished { peer_id, endpoint, ..} => {
                    //     info!("Connection established with {} at {:?}", peer_id, endpoint);
                    //     None
                    // }
                    // SwarmEvent::ConnectionClosed {peer_id, endpoint, ..} => {
                    //     info!("Connection closed with {} at {:?}", peer_id, endpoint);
                    //     None
                    // }
                    // SwarmEvent::IncomingConnection {local_addr, send_back_addr} => {
                    //     info!("Incoming connection from {:?}", send_back_addr);
                    //     None
                    // }
                    // SwarmEvent::Dialing(PeerId) => {
                    //     info!("Dialing peer {}", PeerId);
                    //     None
                    // }
                    _ => {
                        // info!("Unhandled Swarm Event: {:?}", event);
                        None
                    }
                },
            }
        };

        if let Some(event) = evt {
            match event {
                EventType::Response(resp) => {
                    let json = serde_json::to_string(&resp).expect("can jsonify response");
                    swarm
                        .behaviour_mut()
                        .floodsub
                        .publish(TOPIC.clone(), json.as_bytes());
                }
                EventType::Input(line) => match line.as_str() {
                    "LIST PEERS" => handle_list_peers(&mut swarm).await,
                    cmd if cmd.starts_with("GET NFT") => {
                        handle_list_nft_info(cmd, &mut swarm).await
                    }
                    cmd if cmd.starts_with("CREATE NFT") => handle_create_nft_info(cmd).await,
                    _ => error!("unknown command"),
                },
            }
        }
    }
}

async fn handle_list_peers(swarm: &mut Swarm<NFTInfoBehaviour>) {
    info!("Discovered Peers:");
    let nodes = swarm.behaviour().mdns.discovered_nodes();
    let mut unique_peers = HashSet::new();
    for peer in nodes {
        unique_peers.insert(peer);
    }
    unique_peers.iter().for_each(|p| info!("{}", p));
}

async fn handle_list_nft_info(cmd: &str, swarm: &mut Swarm<NFTInfoBehaviour>) {
    let rest = cmd.strip_prefix("GET NFT ");
    match rest {
        Some("ALL") => {
            let req = ListRequest {
                mode: ListMode::ALL,
            };
            let json = serde_json::to_string(&req).expect("can jsonify request");
            swarm
                .behaviour_mut()
                .floodsub
                .publish(TOPIC.clone(), json.as_bytes());
        }
        Some(collection_name) => {
            let req = ListRequest {
                mode: ListMode::Collection(collection_name.to_owned()),
            };
            let json = serde_json::to_string(&req).expect("can jsonify request");
            swarm
                .behaviour_mut()
                .floodsub
                .publish(TOPIC.clone(), json.as_bytes());
        }
        None => {
            let v = read_local_nft_info();
            info!("Local NFTInfo({})", v.len());
            v.iter().for_each(|r| info!("{:?}", r));
        }
    }
}

async fn handle_create_nft_info(cmd: &str) {
    if let Some(rest) = cmd.strip_prefix("CREATE NFT ") {
        let elements: Vec<&str> = rest.split("|").collect();
        if elements.len() < 3 {
            info!("too few arguments - Format: collection_name|item_id|description|owner");
        } else {
            let collection_name = elements.get(0).expect("collection name is present");
            let item_id = elements
                .get(1)
                .expect("item id is present")
                .parse::<u32>()
                .expect("item id parse error");
            let description = elements.get(2).expect("description is present");
            let owner = elements.get(3).expect("owner name is present");
            if let Err(e) = create_new_nft_info(collection_name, item_id, description, owner).await
            {
                error!("error creating NFT info: {}", e);
            };
        }
    }
}
