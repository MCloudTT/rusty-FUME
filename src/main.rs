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
// TODO: Pick a mqtt packet generation/decoding library that is customizable for the purpose of this project and also supports v3,v4 and v5.
use crate::markov::{Mode, StateMachine};
use crate::mqtt::test_connection;
use rand::thread_rng;
use std::env::args;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::info;

mod markov;
pub mod mqtt;
mod process_monitor;
mod runtime;

static PACKET_QUEUE: OnceLock<Arc<RwLock<PacketQueue>>> = OnceLock::new();
#[derive(Debug, PartialEq, Eq, Hash, Default, Clone)]
struct Packet(Vec<u8>);
#[derive(Debug, PartialEq, Eq, Hash, Default)]
struct PacketQueue(Vec<Packet>);

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    let mut rng = thread_rng();
    let mut tcpstream = TcpStream::connect("127.0.0.1:1883").await?;
    test_connection(&mut tcpstream).await?;
    info!("Connection established");
    PACKET_QUEUE
        .set(Arc::new(RwLock::new(PacketQueue::default())))
        .unwrap();
    info!("Starting fuzzing!");
    loop {
        let mut new_tcpstream = TcpStream::connect("127.0.0.1:1883").await?;
        let mut state_machine = StateMachine::new(new_tcpstream);
        state_machine.execute(Mode::MutationGuided, &mut rng).await;
        state_machine.state = markov::State::S0;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    Ok(())
}
