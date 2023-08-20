//! # rusty-FUME
//! rusty-FUME is a fuzzer for the MQTT protocol. It is based on [FUME-Fuzzing-MQTT Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers)
//! and uses markov chains to generate new packet chains. If it discovers a new response behaviour the chain is added to the fuzzing queue.
//! We use [tokio](https://tokio.rs/) for async networking.
//! ## The state machine
//! We implement a State machine with a markov chain. All probabilities are configurable for this process(except the ones with only one option).
//! The state machine is defined as follows for the Mutation Guided Fuzzing:
//! - S0: Initial State: Either goto CONNECT state or select a packet from the queue and go to MUTATION state
//! - CONNECT: Add connect to the current chain and go to ADDING State
//! - ADDING: Either add a new packet(configurable probability for each one) to the chain or go to MUTATION state
//! - MUTATION: Mutate, delete, inject or SEND the current chain
//! - SEND: Send the current chain and either go to Sf or MUTATION state
//! - Sf: Final State
//! And this way for Generation Guided Fuzzing:
//! - S0: Initial State: Goto ADD(CONNECT) state
//! - CONNECT: Add connect to the current chain and go to S1
//! - S1: Either add a new packet or go to S2
//! - S2: Inject/Delete/Mutate the current chain or go to SEND
//! - SEND: Send the current chain and either go to Sf or S2
//! Once they get to S2 they behave the same way.
use std::cmp::min;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;
// TODO: Pick a mqtt packet generation/decoding library that is customizable for the purpose of this project and also supports v3,v4 and v5.
// FIXME: Fix ranges...
// TODO: Run this on my server
// TODO: crtl_c handling
// TODO: Try fuzzing a basic mongoose server?
use crate::markov::MAX_PACKETS;
use crate::mqtt::test_connection;
use crate::process_monitor::start_supervised_process;
use crate::runtime::run_thread;
use clap::{Parser, Subcommand};
use futures::future::join_all;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use serde_with::formats::{CommaSeparator, SpaceSeparator};
use serde_with::hex::Hex;
use serde_with::StringWithSeparator;
use serde_with::{serde_as, DisplayFromStr};
use std::str::FromStr;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, info, trace};

mod markov;
pub mod mqtt;
mod packet_pool;
mod process_monitor;
mod runtime;

// TODO: All threads should also dump their last packets for fast replaying
#[serde_as]
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, Serialize, Deserialize)]
pub struct Packets {
    #[serde_as(as = "[StringWithSeparator::<CommaSeparator, u8>; MAX_PACKETS]")]
    inner: [Vec<u8>; MAX_PACKETS],
}
impl Display for Packets {
    // Hex dump the packets
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for i in 0..MAX_PACKETS {
            if !self.inner[i].is_empty() {
                s.push_str(hex::encode(&self.inner[i]).as_str());
                s.push('\n');
            }
        }
        write!(f, "{}", s)
    }
}
impl Packets {
    pub fn append(&mut self, packet: &mut Vec<u8>) {
        // Search the first free slot and insert it there
        let size = self.size();
        if size < MAX_PACKETS {
            self.inner[size] = packet.clone();
        }
    }
    pub fn is_full(&self) -> bool {
        self.inner.iter().all(|x| !x.is_empty())
    }
    pub fn size(&self) -> usize {
        min(1, self.inner.iter().filter(|x| !x.is_empty()).count())
    }
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

#[serde_as]
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, Serialize, Deserialize)]
struct PacketQueue {
    #[serde_as(as = "BTreeMap<StringWithSeparator::<CommaSeparator, u8>, _>")]
    inner: BTreeMap<Vec<u8>, Packets>,
}

impl PacketQueue {
    async fn read_from_file(path: impl AsRef<Path>) -> color_eyre::Result<Self> {
        let mut content = String::new();
        File::open(path).await?.read_to_string(&mut content).await?;
        let packets: Vec<Packets> = toml::from_str(&content)?;
        let mut queue = Self::default();
        for packet in packets {
            queue.inner.insert(packet.inner[0].clone(), packet);
        }
        Ok(queue)
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommands,
    #[arg(short, long, default_value = "127.0.0.1:1883")]
    target: String,
    #[arg(short, long)]
    broker_command: String,
    // TODO: Make the timeout configurable
    #[arg(short, long, default_value = "200")]
    timeout: u64,
}

#[derive(Subcommand, Debug)]
enum SubCommands {
    // TODO: Do Fuzzing args like threads, chances etc
    Fuzz {
        #[arg(short, long, default_value_t = 100)]
        threads: u64,
    },
    Replay {
        #[arg(short, long, default_value_t = false)]
        sequential: bool,
    },
}
/// Struct to serialize threads once they are done(aka the broker has crashed).
#[derive(Serialize, Deserialize, Debug)]
struct SeedAndIterations {
    pub seed: String,
    pub iterations: String,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    console_subscriber::init();
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let packet_queue = Arc::new(RwLock::new(
        // PacketQueue::read_from_file("./packet_pool.toml").await?,
        PacketQueue::default(),
    ));
    match &cli.subcommand {
        SubCommands::Fuzz { threads } => {
            // This receiver is necessary to dump the packets once the broker is stopped
            let (sender, _) = tokio::sync::broadcast::channel(1);
            let mut subscribers = vec![];
            for _ in 0..*threads {
                subscribers.push(sender.subscribe());
            }
            start_supervised_process(sender, cli.broker_command).await?;
            let address = cli.target.clone();
            let mut tcpstream = TcpStream::connect(&address).await?;
            test_connection(&mut tcpstream).await?;
            info!("Connection established, starting fuzzing!");
            let mut rng = thread_rng();
            let _ = fs::create_dir("./threads").await;
            let mut task_handles = vec![];
            for _ in 0u64..*threads {
                let receiver_clone = subscribers.pop().unwrap();
                let seed: u64 = rng.gen();
                task_handles.push(run_thread(
                    seed,
                    receiver_clone,
                    address.clone(),
                    u64::MAX,
                    packet_queue.clone(),
                ));
            }
            join_all(task_handles).await;
            let serialized_pkg_pool = toml::to_string(&packet_queue.write().await.clone());
            info!("Packet Queue: {:?}", serialized_pkg_pool);
        }
        SubCommands::Replay { sequential } => {
            // Iterate through all fuzzing_{}.txt files and replay them one after another
            let mut files = fs::read_dir("./threads")
                .await
                .expect("Failed to find threads folder. Cannot Replay");
            let mut filtered_files = vec![];
            trace!(
                "Found files: {:?} in folder {:?}",
                files,
                std::env::current_dir()
            );

            while let Some(entry) = files.next_entry().await? {
                let path = entry.path();
                let path_str = path.to_string_lossy().to_string();
                if path_str.starts_with("./threads/fuzzing_") && path_str.ends_with(".txt") {
                    filtered_files.push(path_str);
                }
            }
            trace!("Found {} files", filtered_files.len());
            let (sender, receiver) = tokio::sync::broadcast::channel::<()>(1);
            let mut subscribers = vec![];
            for _ in 0..filtered_files.len() {
                subscribers.push(sender.subscribe());
            }
            start_supervised_process(sender, cli.broker_command).await?;
            let mut tcpstream = TcpStream::connect(&cli.target).await?;
            test_connection(&mut tcpstream).await?;
            debug!("Connection established");
            debug!("Starting replay with {} seeds", filtered_files.len());
            let mut threads = vec![];
            for file in filtered_files {
                let receiver_clone = subscribers.pop().unwrap();
                let seed_and_iterations: SeedAndIterations =
                    toml::from_str(&fs::read_to_string(file).await?)?;
                threads.push(run_thread(
                    u64::from_str(&seed_and_iterations.seed).unwrap(),
                    receiver_clone,
                    cli.target.clone(),
                    u64::from_str(&seed_and_iterations.iterations).unwrap(),
                    packet_queue.clone(),
                ));
            }
            if *sequential {
                for (index, thread) in threads.into_iter().enumerate() {
                    info!("Replaying thread number {}", index);
                    thread.await;
                    if !receiver.is_empty() {
                        info!("Crashing seed found!");
                        break;
                    }
                }
                info!("No crash found :/");
            } else {
                join_all(threads).await;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_serialize_packet_queue() {
        let mut packet_queue = PacketQueue::default();
        packet_queue.inner.insert(vec![0x10], Packets::default());
        let serialized = toml::to_string(&packet_queue).unwrap();
        println!("{}", serialized);
    }
}
