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
//! - S0: Initial State: Goto CONNECT
//! - CONNECT: Add connect to the current chain and go to S1
//! - S1: Either add a new packet or go to S2
//! - S2: Inject/Delete/Mutate the current chain or go to SEND
//! - SEND: Send the current chain and either go to Sf or S2
mod markov;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;
    dotenvy::dotenv().ok();
    Ok(())
}
