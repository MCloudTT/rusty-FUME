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
use crate::markov::MAX_PACKETS;
use crate::packets::PacketQueue;
use clap::{Parser, Subcommand};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub mod markov;
pub mod mqtt;
pub mod network;
mod packet_pool;
pub mod packets;
pub mod process_monitor;
pub mod runtime;
// TODO: Clean up main
// TODO: Try fuzzing a basic mongoose server?
// TODO: Fuzz mosquitto compiled with sanitizers
// TODO: Lib-split to allow benchmarking

/// Struct to serialize threads once they are done(aka the broker has crashed).
#[derive(Serialize, Deserialize, Debug)]
pub struct SeedAndIterations {
    pub seed: String,
    pub iterations: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::Packets;
    #[test]
    fn test_serialize_packet_queue() {
        let mut packet_queue = PacketQueue::default();
        packet_queue.inner.insert(vec![0x10], Packets::default());
        let serialized = toml::to_string(&packet_queue).unwrap();
        println!("{}", serialized);
    }
}
