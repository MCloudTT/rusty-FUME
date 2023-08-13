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

# Recommendations
On Linux to allow the reuse of sockets faster, lower the value of `net.ipv4.tcp_fin_timeout`:
```
sudo sysctl -w net.ipv4.tcp_fin_timeout=50
sudo sysctl -w net.ipv4.tcp_tw_reuse=1
```

## Credits
- [FUME: Fuzzing Message Queuing Telemetry Transport Brokers](https://ieeexplore.ieee.org/abstract/document/9796755)
- [FUME-Fuzzing-MQTT-Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers)