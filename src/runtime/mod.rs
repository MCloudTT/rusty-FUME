use crate::markov::{Mode, StateMachine};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::broadcast::Receiver;
use tokio::{fs, task};
use tracing::info;

// TODO: Change address to allow other kinds of Streams
/// Runs a task that connects to the broker and fuzzes it
pub(crate) fn run_thread(
    i: u64,
    receiver_clone: Receiver<()>,
    address: impl ToSocketAddrs + Clone + Send + Sync + 'static,
) {
    task::spawn(async move {
        let mut last_packets = Vec::new();
        let mut counter: u64 = 0;
        loop {
            // TODO: Don't unwrap here, if we can't connect we should stop the fuzzing and dump packets
            let mut new_tcpstream = TcpStream::connect(address.clone()).await;
            if new_tcpstream.is_err() {
                break;
            }
            let new_tcpstream = new_tcpstream.unwrap();
            let mut state_machine = StateMachine::new(new_tcpstream);
            state_machine
                .execute(
                    Mode::MutationGuided,
                    &mut Xoshiro256Plus::seed_from_u64(i + counter),
                )
                .await;
            last_packets = state_machine.previous_packets.clone();
            // We receive a message once the broker is stopped
            if !receiver_clone.is_empty() {
                break;
            }
        }
        // If the fuzzing is stopped we dump the packets
        for packet in last_packets.iter().enumerate() {
            fs::write(
                format!("fuzzing_{}_{}.txt", i, packet.0),
                packet.1.to_string(),
            )
            .await
            .unwrap();
        }
        info!("Thread {} dumped packets", i);
    });
}
