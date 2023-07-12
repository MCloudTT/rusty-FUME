use crate::markov::ByteStream;
use mqtt::packet::QoSWithPacketIdentifier;
use mqtt::{Encodable, TopicFilter, TopicName};
use rand::prelude::ThreadRng;
use rand::Rng;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::join;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::time::timeout;
use tracing::debug;
/// Macro for importing packets. TODO: They need to be converted from hex to bytes
#[macro_export]
macro_rules! import_packets{
    ($($name:ident),*) => {
        $(
            pub(crate) const $name: &'static str = include_str!(concat!("../../mqtt_corpus/", stringify!($name)));
        )*
    }
}
import_packets!(
    AUTH,
    CONNACK,
    CONNECT,
    DISCONNECT,
    PINGREQ,
    PINGRESP,
    PUBACK,
    PUBCOMP,
    PUBLISH,
    PUBREC,
    PUBREL,
    RESERVED,
    SUBACK,
    SUBSCRIBE,
    UNSUBACK,
    UNSUBSCRIBE
);
pub(crate) fn generate_auth_packet() -> Vec<u8> {
    unimplemented!("Auth packet not implemented yet. Switch to MQTT V5")
}
pub(crate) fn generate_connect_packet() -> Vec<u8> {
    let mut connect = mqtt::packet::ConnectPacket::new("Hello MQTT Broker");
    connect.set_will(Some((
        TopicName::new("topic").unwrap(),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    )));
    let mut packet = Vec::new();
    connect.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_publish_packet() -> Vec<u8> {
    let mut publish = mqtt::packet::PublishPacket::new(
        TopicName::new("topic").unwrap(),
        QoSWithPacketIdentifier::Level0,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    );
    publish.set_retain(true);
    let mut packet = Vec::new();
    publish.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_subscribe_packet() -> Vec<u8> {
    let mut subscribe = mqtt::packet::SubscribePacket::new(
        10,
        vec![(
            TopicFilter::new("topic").unwrap(),
            mqtt::QualityOfService::Level0,
        )],
    );
    let mut packet = Vec::new();
    subscribe.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_unsubscribe_packet() -> Vec<u8> {
    let mut unsubscribe =
        mqtt::packet::UnsubscribePacket::new(10, vec![TopicFilter::new("topic").unwrap()]);
    let mut packet = Vec::new();
    unsubscribe.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_disconnect_packet() -> Vec<u8> {
    let mut disconnect = mqtt::packet::DisconnectPacket::new();
    let mut packet = Vec::new();
    disconnect.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_pingreq_packet() -> Vec<u8> {
    let mut pingreq = mqtt::packet::PingreqPacket::new();
    let mut packet = Vec::new();
    pingreq.encode(&mut packet).unwrap();
    packet
}

pub(crate) async fn test_connection(stream: &mut impl ByteStream) -> color_eyre::Result<()> {
    let packets = CONNECT.split('\n').map(|x| hex::decode(x).unwrap());
    for packet in packets {
        stream.write_all(&packet).await?;
        let mut buf = [0; 1024];
        let _ = timeout(Duration::from_secs(1), stream.read(&mut buf)).await;
        debug!("Packet hex encoded: {:?}", hex::encode(&buf));
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum SendResult {
    // The received packet is a new behavior
    NewBehaviour,
    // The received packet matches a known behavior
    KnownBehaviour,
    // Probably a DOS discovered. The server didn't respond in time
    Timeout,
    // The server crashed. As the connection is closed or similar
    ReceiveErr,
    // We couldn't send the packet. A previous packet might have crashed the server
    SendErr,
}
pub(crate) async fn send_packet(stream: &mut impl ByteStream, packet: &[u8]) -> SendResult {
    let write_result = stream.write_all(packet).await;
    if write_result.is_err() {
        return SendResult::SendErr;
    }
    let mut buf = [0; 1024];
    let res = timeout(Duration::from_millis(100), stream.read(&mut buf)).await;
    match res {
        Ok(Ok(p)) => {
            // TODO: Check if it matches a known packet we received or is a new behavior and if it is add it to the corpus
            debug!("Packet hex encoded: {:?}", hex::encode(&buf));
            SendResult::NewBehaviour
        }
        Err(t) => {
            debug!("Timeout: {:?}", t);
            // TODO: Retry sending the packet
            SendResult::Timeout
        }
        Ok(Err(e)) => {
            debug!("Receive error: {:?}", e);
            SendResult::ReceiveErr
        }
    }
}

fn known_packet(packet: &[u8]) -> bool {
    // TODO: decode the packet and extract user id, payload, topic etc. because those don't matter to see if it is a known packet
    false
}
