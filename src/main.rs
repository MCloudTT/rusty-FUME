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
use crate::mqtt::test_connection;
use crate::network::connect_to_broker;
use crate::packets::PacketQueue;
use crate::process_monitor::start_supervised_process;
use crate::runtime::{iterations_tracker, run_thread};
use clap::{Parser, Subcommand};
use futures::future::join_all;

#[cfg(feature = "stats")]
use opentelemetry::runtime::Tokio;
#[cfg(feature = "stats")]
use opentelemetry::trace::Tracer;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
#[cfg(feature = "stats")]
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::channel as mpsc_channel;
use tokio::sync::RwLock;
use tokio::{fs, task};
use tracing::{debug, info, instrument, span, trace};
#[cfg(feature = "stats")]
use tracing_subscriber::layer::SubscriberExt;
#[cfg(feature = "stats")]
use tracing_subscriber::util::SubscriberInitExt;

mod markov;
pub mod mqtt;
mod network;
mod packet_pool;
mod packets;
mod process_monitor;
mod runtime;
// TODO: Clean up main
// TODO: Try fuzzing a basic mongoose server?
// TODO: Fuzz mosquitto compiled with sanitizers
// TODO: Support TLS, WS and QUIC

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
    #[arg(long, default_value = "200")]
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
    color_eyre::install()?;
    #[cfg(not(feature = "stats"))]
    {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    }
    #[cfg(feature = "stats")]
    let tracer = opentelemetry_jaeger::new_collector_pipeline()
        .with_endpoint(env::var("OPENTELEMETRY_JAEGER_ENDPOINT").unwrap())
        .with_service_name("rusty-fume")
        .with_reqwest()
        .install_batch(Tokio)?;
    #[cfg(feature = "stats")]
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    #[cfg(feature = "stats")]
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(opentelemetry)
        .init();
    start_app().await?;
    #[cfg(feature = "stats")]
    {
        opentelemetry::global::shutdown_tracer_provider();
    }
    Ok(())
}

#[instrument]
async fn start_app() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    let packet_queue = Arc::new(RwLock::new(
        PacketQueue::read_from_file("./packet_pool.toml").await?,
    ));
    match &cli.subcommand {
        SubCommands::Fuzz { threads } => {
            // The channel used for iteration counting
            let (it_sender, it_receiver) = mpsc_channel::<u64>(*threads as usize);
            // This receiver is necessary to dump the packets once the broker is stopped
            let (sender, _) = tokio::sync::broadcast::channel(1);
            let mut subscribers = vec![];
            for _ in 0..*threads {
                subscribers.push(sender.subscribe());
            }
            start_supervised_process(sender, cli.broker_command).await?;
            let address = cli.target.clone();
            let mut stream = connect_to_broker(&cli.target).await?;
            test_connection(&mut stream).await?;
            info!("Connection established, starting fuzzing!");
            let mut rng = thread_rng();
            let _ = fs::create_dir("./threads").await;
            let mut task_handles = vec![];
            for _ in 0u64..*threads {
                let it_sender_clone = it_sender.clone();
                let receiver_clone = subscribers.pop().unwrap();
                let seed: u64 = rng.gen();
                task_handles.push(run_thread(
                    seed,
                    receiver_clone,
                    address.clone(),
                    u64::MAX,
                    packet_queue.clone(),
                    it_sender_clone,
                ));
            }
            // Track it/s
            let threads = *threads as usize;
            task::spawn(async move {
                iterations_tracker(threads, it_receiver).await;
            });
            join_all(task_handles).await;
            let serialized_pkg_pool = toml::to_string(&packet_queue.write().await.clone());
            trace!("Packet Queue: {:?}", serialized_pkg_pool);
        }
        SubCommands::Replay { sequential } => {
            // TODO: When replaying the tasks may get loaded in different order
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
            let mut stream = connect_to_broker(&cli.target).await?;
            test_connection(&mut stream).await?;
            debug!("Connection established");
            debug!("Starting replay with {} seeds", filtered_files.len());
            let mut threads = vec![];
            let unused_it_channel = mpsc_channel::<u64>(filtered_files.len()).0;
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
                    unused_it_channel.clone(),
                ));
            }
            if *sequential {
                for (index, thread) in threads.into_iter().enumerate() {
                    info!("Replaying thread number {}", index);
                    thread.await;
                    if !receiver.is_empty() {
                        info!("Crashing seed found!");
                        return Ok(());
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
    use crate::packets::Packets;
    #[test]
    fn test_serialize_packet_queue() {
        let mut packet_queue = PacketQueue::default();
        packet_queue.inner.insert(vec![0x10], Packets::default());
        let serialized = toml::to_string(&packet_queue).unwrap();
        println!("{}", serialized);
    }
}
