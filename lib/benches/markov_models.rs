use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::sync::Arc;
use tokio::io::duplex;
// This is a struct that tells Criterion.rs to use the "futures" crate's current-thread executor
use lib::markov::{Mode, StateMachine};
use lib::packets::PacketQueue;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

async fn exec_markov_fast(m: Mode) {
    let stream = duplex(10 * 1024).0;
    let mut machine = StateMachine::new(stream);
    let mut rng = Xoshiro256PlusPlus::from_seed([5; 32]);
    machine
        .execute(m, &mut rng, &Arc::new(RwLock::new(PacketQueue::default())))
        .await;
}

fn markov_model_execution(c: &mut Criterion) {
    for mode in [Mode::MutationGuided, Mode::GenerationGuided] {
        c.bench_with_input(
            BenchmarkId::new("execute_markov_model", mode),
            &mode,
            |b, &m| {
                b.to_async(Runtime::new().unwrap())
                    .iter(|| exec_markov_fast(m));
            },
        );
    }
}

criterion_group!(benches, markov_model_execution);
criterion_main!(benches);
