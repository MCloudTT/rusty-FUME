use crate::markov::StateMachine;
use crate::network::connect_to_broker;
use crate::packets::PacketQueue;
use crate::SeedAndIterations;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::{fs, task};
use tracing::*;

/// Runs a task that connects to the broker and fuzzes it
pub async fn run_thread(
    seed: u64,
    receiver_clone: Receiver<()>,
    address: String,
    iterations: u64,
    packet_queue: Arc<RwLock<PacketQueue>>,
    it_sender_clone: Sender<u64>,
    timeout: u16,
) {
    let task_handle = task::spawn(async move {
        let mut last_packets = Vec::new();
        let mut counter: u64 = 0;
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        while counter < iterations {
            let new_stream = connect_to_broker(&address.clone()).await;
            if new_stream.is_err() {
                // Workaround for connections not being closed fast enough. See https://stackoverflow.com/questions/76238841/cant-assign-requested-address-in-request
                error!(
                    "Error connecting to broker: {:?}. See recommendations",
                    new_stream
                );
                if !receiver_clone.is_empty() {
                    break;
                }
                // So we'll just have a "back-off" sleep here
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            let new_tcpstream = new_stream.unwrap();
            let mut state_machine = StateMachine::new(new_tcpstream, timeout);
            let mode = rng.gen();
            state_machine.execute(mode, &mut rng, &packet_queue).await;
            last_packets = state_machine.previous_packets.clone();
            // We receive a message once the broker is stopped
            if !receiver_clone.is_empty() {
                break;
            }
            counter += 1;
            if counter % 5000 == 0 {
                // Display iterations per second
                let _ = it_sender_clone.send(counter).await;
            }
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
        // Dump the packet we crashed on
        let _ = fs::create_dir("crashes").await;
        let _ = fs::write(
            format!("crashes/crash_{}.txt", seed),
            format!("{:?}", last_packets),
        )
        .await;

        info!("Thread {seed} finished at {counter} iterations, when {iterations} were the target!");
    });
    let _ = task_handle.await;
}

pub async fn iterations_tracker(threads: usize, mut it_receiver: MpscReceiver<u64>) {
    let mut last_iterations = 0;
    loop {
        let start = std::time::Instant::now();
        let mut iteration_buffer = vec![0; threads];
        for i in 1..threads {
            let value = it_receiver.recv().await;
            match value {
                Some(v) => iteration_buffer[i] = v,
                None => break,
            }
        }
        let sum: u64 = iteration_buffer.iter().sum();
        let elapsed = start.elapsed().as_millis();
        let it_per_second = (sum.saturating_sub(last_iterations)) as f64 / elapsed as f64 * 1000f64;
        info!("{} it/s", it_per_second);
        last_iterations = sum;
    }
}
