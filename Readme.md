# rusty-FUME
A high-performance MQTT network Fuzzer.
This is an implementation of [FUME-Fuzzing-MQTT-Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers/) in Rust.

# Running the project
After [installing Rust](https://rustup.rs), run the following command in the project directory:
```
cargo run --release -- fuzz
```
## Credits
- [FUME: Fuzzing Message Queuing Telemetry Transport Brokers](https://ieeexplore.ieee.org/abstract/document/9796755)
- [FUME-Fuzzing-MQTT-Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers)