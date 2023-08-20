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
**Note: DO NOT USE THIS ON A PRODUCTION SERVER AS IT MAY HAVE UNINTENDED SIDE EFFECTS**
That being said, it works fine on my local machine. I recommend running the following commands before fuzzing:
```
sudo sysctl -w net.ipv4.tcp_fin_timeout=1
sudo sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
```

## Credits
- [FUME: Fuzzing Message Queuing Telemetry Transport Brokers](https://ieeexplore.ieee.org/abstract/document/9796755)
- [FUME-Fuzzing-MQTT-Brokers](https://github.com/PBearson/FUME-Fuzzing-MQTT-Brokers)