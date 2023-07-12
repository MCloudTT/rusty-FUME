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
mod mutations;

use crate::markov::mutations::{delete, inject, swap, InjectType};
use crate::markov::Mode::MutationGuided;
use crate::mqtt::{
    generate_auth_packet, generate_connect_packet, generate_disconnect_packet,
    generate_pingreq_packet, generate_publish_packet, generate_subscribe_packet,
    generate_unsubscribe_packet, send_packet, SendResult,
};
use crate::PACKET_QUEUE;
use rand::distributions::Standard;
use rand::prelude::{Distribution, ThreadRng};
use rand::Rng;
use std::fmt::Debug;
use std::process::exit;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

const SEL_FROM_QUEUE: f32 = 0.5;
const PACKET_CHANCE: f32 = 1. / 15.;
const SEND_CHANCE: f32 = 0.33;
const BOF_CHANCE: f32 = 0.25;
const MUT_AFTER_SEND: f32 = 0.5;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum State {
    S0,
    ADD(PacketType),
    ADDING,
    SelectFromQueue,
    MUTATION,
    Mutate(Mutations),
    SEND,
    Sf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mutations {
    // Inserts bytes into the payload
    Inject(InjectType),
    // Deletes bytes from the payload
    Delete,
    // Changes bytes in the payload
    Swap,
}
impl Distribution<Mutations> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Mutations {
        match rng.gen_range(0..3) {
            0 => Mutations::Inject(rng.gen()),
            1 => Mutations::Delete,
            2 => Mutations::Swap,
            _ => unreachable!(),
        }
    }
}

pub trait ByteStream: AsyncReadExt + AsyncWriteExt + Unpin + Debug {}

impl<T> ByteStream for T where T: AsyncReadExt + AsyncWriteExt + Unpin + Debug {}

pub struct StateMachine<B>
where
    B: ByteStream,
{
    // The current state of the state machine
    pub(crate) state: State,
    // The current packet in bytes
    packet: Vec<u8>,
    // The current stream, TlsStream TcpStream or WebsocketStream
    stream: B,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Mode {
    MutationGuided,
    GenerationGuided,
}
impl<B> StateMachine<B>
where
    B: ByteStream,
{
    pub(crate) fn new(stream: B) -> Self {
        Self {
            state: State::S0,
            // TODO: Do alloc upfront. Remember to adjust the len usage in the mutations
            packet: Vec::new(),
            stream,
        }
    }
    pub(crate) async fn execute(&mut self, mode: Mode, rng: &mut ThreadRng) {
        while self.state != State::Sf {
            self.next(mode, rng).await;
            debug!("State: {:?}", self.state);
            debug!("Packet len {}", self.packet.len());
        }
    }
    async fn next(&mut self, mode: Mode, rng: &mut ThreadRng) {
        match &self.state {
            State::S0 => {
                if mode == MutationGuided && rng.gen_range(0f32..1f32) > SEL_FROM_QUEUE {
                    self.state = State::SelectFromQueue;
                } else {
                    self.state = State::ADD(PacketType::CONNECT);
                }
            }
            State::SelectFromQueue => {
                // Maybe we should use a priority queue in-memory here instead of storing on disk(overhead). Should be measured in the future.
                let queue = PACKET_QUEUE.get().unwrap().read().await;
                if queue.0.is_empty() {
                    self.state = State::ADD(PacketType::CONNECT);
                } else {
                    let packet = queue.0[rng.gen_range(0..queue.0.len())].clone();
                    self.packet = packet.0;
                    self.state = State::MUTATION;
                }
            }
            State::ADD(packet_type) => {
                match packet_type {
                    PacketType::CONNECT => {
                        self.packet.append(&mut generate_connect_packet());
                    }
                    PacketType::PUBLISH => {
                        self.packet.append(&mut generate_publish_packet());
                    }
                    PacketType::SUBSCRIBE => {
                        self.packet.append(&mut generate_subscribe_packet());
                    }
                    PacketType::UNSUBSCRIBE => {
                        self.packet.append(&mut generate_unsubscribe_packet());
                    }
                    PacketType::PINGREQ => {
                        self.packet.append(&mut generate_pingreq_packet());
                    }
                    PacketType::DISCONNECT => {
                        self.packet.append(&mut generate_disconnect_packet());
                    }
                    _ => unreachable!(),
                }
                self.state = State::ADDING
            }
            State::ADDING => {
                if rng.gen_range(0f32..1f32) < PACKET_CHANCE {
                    self.state = State::ADD(rng.gen());
                } else {
                    self.state = State::MUTATION;
                }
            }
            State::MUTATION => {
                if rng.gen_range(0f32..1f32) < SEND_CHANCE {
                    self.state = State::SEND;
                } else {
                    self.state = State::Mutate(rng.gen());
                }
            }
            State::Mutate(mutation) => {
                match mutation {
                    Mutations::Inject(t) => {
                        inject(&mut self.packet, rng, t);
                    }
                    Mutations::Delete => {
                        delete(&mut self.packet, rng);
                    }
                    Mutations::Swap => {
                        swap(&mut self.packet, rng);
                    }
                }
                self.state = State::MUTATION;
            }
            State::SEND => {
                let res = send_packet(&mut self.stream, &self.packet).await;
                match res {
                    SendResult::Timeout => {
                        // Write the packet to disk and exit
                        fs::write("crashing_packet", &self.packet)
                            .await
                            .expect("Unable to write file");
                        exit(-1);
                    }
                    SendResult::SendErr => {
                        // TODO: Here we should "ask" if the server has exited. If so it probably crashed and we should exit
                    }
                    _ => {}
                }
                if rng.gen_range(0f32..1f32) < MUT_AFTER_SEND {
                    self.state = State::Mutate(rng.gen());
                } else {
                    self.state = State::ADD(rng.gen());
                }
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BOF {
    BOF,
    NOBOF,
}

/// The MQTT Packet types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PacketType {
    AUTH,
    CONNACK,
    CONNECT,
    DISCONNECT,
    PINGREQ,
    PINGRESP,
    PUBACK,
    PUBCOMP,
    PUBLISH,
    PUBREC,
    PUBREL,
    RESERVED,
    SUBACK,
    SUBSCRIBE,
    UNSUBACK,
    UNSUBSCRIBE,
}

// Implement a distribution for the packet types
impl Distribution<PacketType> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> PacketType {
        match rng.gen_range(0..6) {
            0 => PacketType::CONNECT,
            1 => PacketType::PUBLISH,
            2 => PacketType::SUBSCRIBE,
            3 => PacketType::UNSUBSCRIBE,
            4 => PacketType::PINGREQ,
            5 => PacketType::DISCONNECT,
            _ => unreachable!(),
        }
    }
}
