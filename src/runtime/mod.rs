use crate::markov::{Mode, StateMachine};
use crate::SeedAndIterations;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::broadcast::Receiver;
use tokio::{fs, task};
use tracing::{error, info};

// TODO: Change address to allow other kinds of Streams
/// Runs a task that connects to the broker and fuzzes it
pub(crate) async fn run_thread(
    seed: u64,
    receiver_clone: Receiver<()>,
    address: impl ToSocketAddrs + Clone + Send + Sync + 'static,
    iterations: u64,
) {
    let task_handle = task::spawn(async move {
        let mut last_packets = Vec::new();
        let mut counter: u64 = 0;
        while counter <= iterations {
            let new_tcpstream = TcpStream::connect(address.clone()).await;
            if new_tcpstream.is_err() {
                break;
            }
            let new_tcpstream = new_tcpstream.unwrap();
            let mut state_machine = StateMachine::new(new_tcpstream);
            state_machine
                .execute(
                    Mode::MutationGuided,
                    &mut Xoshiro256Plus::seed_from_u64(seed.wrapping_add(counter)),
                )
                .await;
            last_packets = state_machine.previous_packets.clone();
            // We receive a message once the broker is stopped
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
        info!("Thread {} finished!", seed);
    });
    let _ = task_handle.await;
}
