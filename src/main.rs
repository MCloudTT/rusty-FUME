use clap::{Parser, Subcommand};
use futures::future::join_all;
use lib::mqtt::test_conn_from_address;
use lib::packets::PacketQueue;
use lib::process_monitor::start_supervised_process;
use lib::runtime::{iterations_tracker, run_thread};
use lib::SeedAndIterations;
use rand::{thread_rng, Rng};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::channel as mpsc_channel;
use tokio::sync::RwLock;
use tokio::{fs, task};
use tracing::{debug, info, trace};

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
    timeout: u16,
}

#[derive(Subcommand, Debug)]
enum SubCommands {
    Fuzz {
        #[arg(short, long, default_value_t = 100)]
        threads: u64,
    },
    Replay {
        #[arg(short, long, default_value_t = false)]
        sequential: bool,
    },
}
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    console_subscriber::init();
    color_eyre::install()?;
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
            test_conn_from_address(&address, cli.timeout).await?;
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
                    cli.timeout,
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
            let mut filtered_files = collect_threads().await;
            trace!("Found {} files", filtered_files.len());
            let (sender, receiver) = tokio::sync::broadcast::channel::<()>(1);
            let mut subscribers = vec![];
            for _ in 0..filtered_files.len() {
                subscribers.push(sender.subscribe());
            }
            start_supervised_process(sender, cli.broker_command).await?;
            test_conn_from_address(&cli.target, cli.timeout).await?;
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
                    cli.timeout,
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

async fn collect_threads() -> Vec<String> {
    let mut files = fs::read_dir("./threads")
        .await
        .expect("Failed to find threads folder. Cannot Replay");
    let mut filtered_files = vec![];
    trace!(
        "Found files: {:?} in folder {:?}",
        files,
        std::env::current_dir()
    );

    while let Ok(Some(entry)) = files.next_entry().await {
        let path = entry.path();
        let path_str = path.to_string_lossy().to_string();
        if path_str.starts_with("./threads/fuzzing_") && path_str.ends_with(".txt") {
            filtered_files.push(path_str);
        }
    }
    filtered_files
}
