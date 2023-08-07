# rusty-FUME
A high-performance MQTT network Fuzzer.
This is an implementation of [FUME-Fuzzing-MQTT-Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers/) in Rust.

# Running the project
After [installing Rust](https://rustup.rs), run the following command in the project directory:
```
cargo run -r -- --broker-command "YOUR_BROKER_START_COMMAND" fuzz
```
After fuzzing has found a crash you can run the following command to reproduce the crash:
```
cargo run -r -- --broker-command "YOUR_BROKER_START_COMMAND" replay
```

## Credits
- [FUME: Fuzzing Message Queuing Telemetry Transport Brokers](https://ieeexplore.ieee.org/abstract/document/9796755)
- [FUME-Fuzzing-MQTT-Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers)