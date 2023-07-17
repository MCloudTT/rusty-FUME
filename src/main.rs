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
use std::fmt::Display;
// TODO: Pick a mqtt packet generation/decoding library that is customizable for the purpose of this project and also supports v3,v4 and v5.
// FIXME: Fix ranges...
use crate::markov::{Mode, StateMachine, MAX_PACKETS};
use crate::mqtt::{send_packet, test_connection};
use crate::process_monitor::start_supervised_process;
use crate::runtime::run_thread;
use clap::{Args, Parser, Subcommand};
use rand::{thread_rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpStream;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::RwLock;
use tokio::{fs, task};
use tracing::{debug, info, trace};

mod markov;
pub mod mqtt;
mod process_monitor;
mod runtime;

static PACKET_QUEUE: OnceLock<Arc<RwLock<PacketQueue>>> = OnceLock::new();
#[derive(Debug, PartialEq, Eq, Hash, Default, Clone)]
pub struct Packets([Vec<u8>; MAX_PACKETS]);
impl Display for Packets {
    // Hex dump the packets
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for i in 0..MAX_PACKETS {
            if !self.0[i].is_empty() {
                s.push_str(hex::encode(&self.0[i]).as_str());
                s.push('\n');
            }
        }
        write!(f, "{}", s)
    }
}
impl Packets {
    pub fn append(&mut self, packet: &mut Vec<u8>) {
        // Search the first free slot and insert it there
        for i in 0..MAX_PACKETS {
            if self.0[i].is_empty() {
                self.0[i] = packet.clone();
                return;
            }
        }
    }
    pub fn is_full(&self) -> bool {
        self.0.iter().all(|x| !x.is_empty())
    }
    pub fn size(&self) -> usize {
        min(1, self.0.iter().filter(|x| !x.is_empty()).count())
    }
    pub fn new() -> Self {
        Self(Default::default())
    }
}
#[derive(Debug, PartialEq, Eq, Hash, Default)]
struct PacketQueue(Vec<Packets>);

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommands,
    #[arg(short, long, default_value = "127.0.0.1:1883")]
    target: String,
}

#[derive(Subcommand, Debug)]
enum SubCommands {
    // TODO: Do Fuzzing args like threads, chances etc
    Fuzz,
    Replay,
}
// TODO: Main has gotten too complicated, refactor it to runtime/mod.rs
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    match &cli.subcommand {
        SubCommands::Fuzz => {
            let threads = 100;
            // This receiver is necessary to dump the packets once the broker is stopped
            let (sender, _) = tokio::sync::broadcast::channel(1);
            let mut subscribers = vec![];
            for _ in 0..threads {
                subscribers.push(sender.subscribe());
            }
            start_supervised_process(sender).await?;
            // TODO: Make the IP configurable via clap or similar
            let address = cli.target.clone();
            let mut tcpstream = TcpStream::connect(&address).await?;
            test_connection(&mut tcpstream).await?;
            info!("Connection established");
            PACKET_QUEUE
                .set(Arc::new(RwLock::new(PacketQueue::default())))
                .unwrap();
            info!("Starting fuzzing!");
            for i in 0..threads {
                let receiver_clone = subscribers.pop().unwrap();
                run_thread(i, receiver_clone, address.clone());
            }
            loop {}
        }
        SubCommands::Replay => {
            // Iterate through all fuzzing_{}.txt files and replay them one after another
            let mut files = fs::read_dir("./").await?;
            let mut filtered_files = vec![];
            trace!(
                "Found files: {:?} in folder {:?}",
                files,
                std::env::current_dir()
            );

            while let Some(entry) = files.next_entry().await? {
                let path = entry.path();
                let path_str = path.to_string_lossy().to_string();
                if path_str.starts_with("./fuzzing_") && path_str.ends_with(".txt") {
                    filtered_files.push(path_str);
                }
            }
            trace!("Found {} files", filtered_files.len());
            let (sender, receiver) = tokio::sync::broadcast::channel::<()>(1);
            start_supervised_process(sender).await?;
            let mut tcpstream = TcpStream::connect(&cli.target).await?;
            test_connection(&mut tcpstream).await?;
            debug!("Connection established");
            let mut all_packets: Vec<Packets> = vec![];
            for file in filtered_files {
                let content = fs::read_to_string(file).await?;
                let mut packets = Packets::new();
                for line in content.lines() {
                    let packet = hex::decode(line)?;
                    packets.append(&mut packet.clone());
                }
                all_packets.push(packets);
            }
            info!("Starting replay with {} packet chains!", all_packets.len());
            for packets in &all_packets {
                let mut stream = TcpStream::connect(&cli.target).await?;
                for packet in &packets.0 {
                    let _ = send_packet(&mut stream, &packet).await;
                }
                if !receiver.is_empty() {
                    info!("Found crashing packet chain, dumping to crashing_packet.txt");
                    fs::write("crashing_packet.txt", packets.to_string()).await?;
                    break;
                }
            }
            info!("No crashing packet chain found!");
        }
    }
    Ok(())
}
