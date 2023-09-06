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
use crate::markov::Mode::{GenerationGuided, MutationGuided};
use crate::mqtt::{
    generate_connect_packet, generate_disconnect_packet, generate_pingreq_packet,
    generate_publish_packet, generate_subscribe_packet, generate_unsubscribe_packet, send_packets,
    SendError,
};
use crate::packets::{PacketQueue, Packets};
use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::default::Default;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::*;

const SEL_FROM_QUEUE: f32 = 0.7;
const PACKET_APPEND_CHANCE: f32 = 0.2;
const SEND_CHANCE: f32 = 0.2;
const BOF_CHANCE: f32 = 0.2;
const MUT_AFTER_SEND: f32 = 0.7;
pub const MAX_PACKETS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum State {
    #[default]
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

pub trait ByteStream: AsyncReadExt + AsyncWriteExt + Unpin + Debug + Send {}

impl<T> ByteStream for T where T: AsyncReadExt + AsyncWriteExt + Unpin + Debug + Send {}
pub struct StateMachine<B>
where
    B: ByteStream,
{
    // The current state of the state machine
    pub(crate) state: State,
    // The current packet in bytes
    packets: Packets,
    // Previous packets. Useful for dumping the packets to disk(For tracking errors)
    pub previous_packets: Vec<Packets>,
    // The current stream, TlsStream TcpStream or WebsocketStream
    stream: B,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Mode {
    MutationGuided,
    GenerationGuided,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutationGuided => write!(f, "Mutation Guided"),
            GenerationGuided => write!(f, "Generation Guided"),
        }
    }
}

impl Distribution<Mode> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Mode {
        match rng.gen_range(0..10) {
            0..=4 => MutationGuided,
            5..=9 => GenerationGuided,
            _ => unreachable!(),
        }
    }
}
impl<B> StateMachine<B>
where
    B: ByteStream,
{
    pub fn new(stream: B) -> Self {
        Self {
            stream,
            state: Default::default(),
            packets: Packets::new(),
            previous_packets: Vec::new(),
        }
    }
    pub async fn execute(
        &mut self,
        mode: Mode,
        rng: &mut Xoshiro256PlusPlus,
        packet_queue: &Arc<RwLock<PacketQueue>>,
    ) {
        while self.state != State::Sf {
            self.next(mode, rng, packet_queue).await;
            trace!("State: {:?}", self.state);
        }
    }
    async fn next(
        &mut self,
        mode: Mode,
        rng: &mut Xoshiro256PlusPlus,
        packet_queue: &Arc<RwLock<PacketQueue>>,
    ) {
        match &self.state {
            State::S0 => match mode {
                MutationGuided => {
                    if rng.gen_range(0f32..1f32) < SEL_FROM_QUEUE && !self.packets.is_full() {
                        self.state = State::ADD(PacketType::CONNECT);
                    } else {
                        self.state = State::SelectFromQueue;
                    }
                }
                GenerationGuided => {
                    self.state = State::ADD(PacketType::CONNECT);
                }
            },
            State::SelectFromQueue => {
                // Maybe we should use a priority queue in-memory here instead of storing on disk(overhead). Should be measured in the future.
                if packet_queue.read().await.inner.is_empty() {
                    self.state = State::ADD(PacketType::CONNECT);
                } else {
                    let packet_queue_read = packet_queue.read().await;
                    let packet_index = rng.gen_range(0..packet_queue_read.inner.len());
                    let packet = packet_queue_read.inner.iter().nth(packet_index).unwrap();
                    // TODO: Really?
                    self.packets = packet.1.clone().clone();
                    self.state = State::MUTATION;
                }
            }
            State::ADD(packet_type) => {
                match packet_type {
                    PacketType::CONNECT => {
                        self.packets.append(&generate_connect_packet());
                    }
                    PacketType::PUBLISH => {
                        self.packets.append(&generate_publish_packet());
                    }
                    PacketType::SUBSCRIBE => {
                        self.packets.append(&generate_subscribe_packet());
                    }
                    PacketType::UNSUBSCRIBE => {
                        self.packets.append(&generate_unsubscribe_packet());
                    }
                    PacketType::PINGREQ => {
                        self.packets.append(&generate_pingreq_packet());
                    }
                    PacketType::DISCONNECT => {
                        self.packets.append(&generate_disconnect_packet());
                    }
                    _ => unreachable!(),
                }
                self.state = State::ADDING
            }
            State::ADDING => {
                if rng.gen_range(0f32..1f32) < PACKET_APPEND_CHANCE {
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
                        inject(&mut self.packets, rng, t);
                    }
                    Mutations::Delete => {
                        delete(&mut self.packets, rng);
                    }
                    Mutations::Swap => {
                        swap(&mut self.packets, rng);
                    }
                }
                self.state = State::MUTATION;
            }
            State::SEND => {
                self.previous_packets.push(self.packets.clone());
                let res = send_packets(&mut self.stream, &self.packets, packet_queue).await;
                if let Err(e) = res {
                    match e {
                        SendError::Timeout => {}
                        SendError::ReceiveErr => {
                            trace!("Receive error, probably disconnected by the broker...")
                        }
                        SendError::SendErr => {
                            trace!("Send error, probably disconnected by the broker...")
                        }
                    }
                } else {
                    trace!("Sent packet successfully");
                }
                if rng.gen_range(0f32..1f32) > MUT_AFTER_SEND || res.is_err() {
                    self.state = State::Sf;
                } else {
                    self.state = State::Mutate(rng.gen());
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
