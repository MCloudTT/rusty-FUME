use crate::markov::StateMachine;
use crate::{PacketQueue, SeedAndIterations};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::{fs, task};
use tracing::{debug, error, info};

// TODO: Change address to allow other kinds of Streams
/// Runs a task that connects to the broker and fuzzes it
pub(crate) async fn run_thread(
    seed: u64,
    receiver_clone: Receiver<()>,
    address: impl ToSocketAddrs + Clone + Send + Sync + 'static,
    iterations: u64,
    packet_queue: Arc<RwLock<PacketQueue>>,
) {
    let task_handle = task::spawn(async move {
        let mut last_packets = Vec::new();
        let mut counter: u64 = 0;
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        while counter <= iterations {
            let new_tcpstream = TcpStream::connect(address.clone()).await;
            if new_tcpstream.is_err() {
                // Workaround for connections not being closed fast enough. See https://stackoverflow.com/questions/76238841/cant-assign-requested-address-in-request
                error!(
                    "Error connecting to broker: {:?}. See recommendations",
                    new_tcpstream
                );
                if !receiver_clone.is_empty() {
                    break;
                }
                // So we'll just have a "back-off" sleep here
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            let new_tcpstream = new_tcpstream.unwrap();
            let mut state_machine = StateMachine::new(new_tcpstream);
            let mode = rng.gen();
            state_machine.execute(mode, &mut rng, &packet_queue).await;
            last_packets = state_machine.previous_packets.clone();
            // We receive a message once the broker is stopped
            // TODO: Also save last packets upon crash
            if !receiver_clone.is_empty() {
                break;
            }
            counter += 1;
        }
        if iterations == u64::MAX {
            // If the fuzzing is stopped we dump the packets
            let serialized = toml::to_string(&SeedAndIterations {
                seed: seed.to_string(),
                iterations: counter.to_string(),
            })
            .unwrap();
            let res = fs::write(format!("threads/fuzzing_{}.txt", seed), serialized).await;

            // TODO: Handle some errors
            if res.is_err() {
                error!("Error dumping packets: {:?}", res);
            }
        }
        info!("Thread {seed} finished at {counter} iterations, when {iterations} were the target!");
    });
    let _ = task_handle.await;
}
