use crate::markov::ByteStream;
use crate::{Packets, PACKET_QUEUE};
use mqtt::packet::QoSWithPacketIdentifier;
use mqtt::{Encodable, TopicFilter, TopicName};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, trace};
// TODO: Maybe we can begin the packet queue with some more interesting packets that triggered bugs in the past from CVEs
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
    let subscribe = mqtt::packet::SubscribePacket::new(
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
    let unsubscribe =
        mqtt::packet::UnsubscribePacket::new(10, vec![TopicFilter::new("topic").unwrap()]);
    let mut packet = Vec::new();
    unsubscribe.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_disconnect_packet() -> Vec<u8> {
    let disconnect = mqtt::packet::DisconnectPacket::new();
    let mut packet = Vec::new();
    disconnect.encode(&mut packet).unwrap();
    packet
}

pub(crate) fn generate_pingreq_packet() -> Vec<u8> {
    let pingreq = mqtt::packet::PingreqPacket::new();
    let mut packet = Vec::new();
    pingreq.encode(&mut packet).unwrap();
    packet
}

pub(crate) async fn test_connection(stream: &mut impl ByteStream) -> color_eyre::Result<()> {
    stream
        .write_all(generate_connect_packet().as_slice())
        .await?;
    let mut buf = [0; 1024];
    let _ = timeout(Duration::from_secs(1), stream.read(&mut buf)).await;
    debug!("Received Packet hex encoded: {:?}", hex::encode(&buf));
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SendError {
    // Probably a DOS discovered. The server didn't respond in time
    Timeout,
    // The server crashed. As the connection is closed or similar
    ReceiveErr,
    // We couldn't send the packet. A previous packet might have crashed the server
    SendErr,
}

pub(crate) async fn send_packets(
    stream: &mut impl ByteStream,
    packets: &Packets,
) -> Result<(), SendError> {
    for packet in &packets.0 {
        send_packet(stream, packet.as_slice(), packets).await?;
    }
    Ok(())
}

const PACKET_TIMEOUT: u64 = 100;

pub(crate) async fn send_packet(
    stream: &mut impl ByteStream,
    packet: &[u8],
    packets: &Packets,
) -> Result<(), SendError> {
    let write_result = timeout(
        Duration::from_millis(PACKET_TIMEOUT),
        stream.write_all(packet),
    )
    .await;
    match write_result {
        Ok(Ok(_)) => (),
        Err(t) => {
            error!("Timeout: {:?}", t);
        }
        Ok(Err(e)) => {
            debug!("Send error: {:?}", e);
            return Err(SendError::SendErr);
        }
    }
    let mut buf = [0; 1024];
    let res = timeout(Duration::from_millis(PACKET_TIMEOUT), stream.read(&mut buf)).await;
    match res {
        Ok(Ok(p)) => {
            known_packet(&buf[..p], &packets).await;
            Ok(())
        }
        Err(t) => {
            error!("Timeout: {:?}", t);
            // TODO: Retry sending the packet
            Err(SendError::Timeout)
        }
        Ok(Err(e)) => {
            error!("Receive error: {:?}", e);
            Err(SendError::ReceiveErr)
        }
    }
}

/// This works by using the response packet as the key in a hashmap. If the packet is already in the hashmap we know that we have seen it before
async fn known_packet(response_packet: &[u8], input_packet: &Packets) -> bool {
    // TODO: decode the packet and extract user id, payload, topic etc. because those don't matter to see if it is a known packet
    // TODO: More efficient algorithm, maybe Locational Hashing?
    let mut queue_lock = PACKET_QUEUE.get().unwrap().clone();
    let mut queue = queue_lock.write().await;
    if queue
        .0
        .insert(response_packet.to_vec(), input_packet.clone())
        .is_none()
    {
        debug!("New behavior discovered: {:?}", input_packet);
        return true;
    }
    trace!("Known behavior. We have {} known behaviors", queue.0.len());
    false
}
