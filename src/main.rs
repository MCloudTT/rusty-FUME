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
// TODO: Pick a mqtt packet generation/decoding library that is customizable for the purpose of this project and also supports v3,v4 and v5.
// FIXME: Fix ranges...
use crate::markov::{Mode, StateMachine, MAX_PACKETS};
use crate::mqtt::test_connection;
use crate::process_monitor::start_supervised_process;
use rand::{thread_rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::task;
use tracing::info;

mod markov;
pub mod mqtt;
mod process_monitor;
mod runtime;

static PACKET_QUEUE: OnceLock<Arc<RwLock<PacketQueue>>> = OnceLock::new();
#[derive(Debug, PartialEq, Eq, Hash, Default, Clone)]
pub struct Packets([Vec<u8>; MAX_PACKETS]);
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

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    let mut rng = thread_rng();
    start_supervised_process().await?;
    // TODO: Make the IP configurable via clap
    let address = "127.0.0.1:1883";
    let mut tcpstream = TcpStream::connect(address).await?;
    test_connection(&mut tcpstream).await?;
    info!("Connection established");
    PACKET_QUEUE
        .set(Arc::new(RwLock::new(PacketQueue::default())))
        .unwrap();
    info!("Starting fuzzing!");
    for i in 0..100 {
        task::spawn(async move {
            loop {
                let mut new_tcpstream = TcpStream::connect(address.clone()).await.unwrap();
                let mut state_machine = StateMachine::new(new_tcpstream);
                state_machine
                    .execute(Mode::MutationGuided, &mut Xoshiro256Plus::seed_from_u64(i))
                    .await;
            }
        });
    }
    loop {}
    Ok(())
}
