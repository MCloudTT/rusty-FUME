use crate::markov::ByteStream;
use crate::network::connect_to_broker;
use crate::packets::{PacketQueue, Packets};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, info, trace};

pub(crate) fn generate_connect_packet() -> [u8; 62] {
    [
        16, 60, 0, 4, 77, 81, 84, 84, 4, 4, 0, 0, 0, 17, 72, 101, 108, 108, 111, 32, 77, 81, 84,
        84, 32, 66, 114, 111, 107, 101, 114, 0, 5, 116, 111, 112, 105, 99, 0, 22, 1, 2, 3, 4, 5, 6,
        7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 72, 255, 50, 0, 0, 0,
    ]
}

pub(crate) fn generate_publish_packet() -> [u8; 27] {
    [
        49, 25, 0, 5, 116, 111, 112, 105, 99, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 72, 255, 50,
        0, 0, 0,
    ]
}

pub(crate) fn generate_subscribe_packet() -> [u8; 12] {
    [130, 10, 0, 100, 0, 5, 116, 111, 112, 105, 99, 0]
}

pub(crate) fn generate_unsubscribe_packet() -> [u8; 11] {
    [162, 9, 0, 10, 0, 5, 116, 111, 112, 105, 99]
}

pub(crate) fn generate_disconnect_packet() -> [u8; 2] {
    [224, 0]
}

pub(crate) fn generate_pingreq_packet() -> [u8; 2] {
    [192, 0]
}

pub async fn test_conn_from_address(address: &str, timeout: u16) -> color_eyre::Result<()> {
    let mut stream = connect_to_broker(address, timeout).await?;
    test_connection(&mut stream).await?;
    Ok(())
}

async fn test_connection(stream: &mut impl ByteStream) -> color_eyre::Result<()> {
    stream
        .write_all(generate_connect_packet().as_slice())
        .await?;
    let mut buf = [0; 1024];
    let _ = timeout(Duration::from_secs(1), stream.read(&mut buf)).await;
    debug!("Connection established");
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
    packet_queue: &Arc<RwLock<PacketQueue>>,
    timeout: u16,
) -> Result<(), SendError> {
    for packet in packets.inner.iter().filter(|p| !p.is_empty()) {
        send_packet(stream, packet.as_slice(), packets, packet_queue, timeout).await?;
    }
    Ok(())
}

pub(crate) async fn send_packet(
    stream: &mut impl ByteStream,
    packet: &[u8],
    packets: &Packets,
    packet_queue: &Arc<RwLock<PacketQueue>>,
    timeout_ms: u16,
) -> Result<(), SendError> {
    let write_result = timeout(
        Duration::from_millis(timeout_ms as u64),
        stream.write_all(packet),
    )
    .await;
    match write_result {
        Ok(Ok(_)) => (),
        Err(t) => {
            trace!("Timeout: {:?}", t);
        }
        Ok(Err(e)) => {
            trace!("Send error: {:?}", e);
            return Err(SendError::SendErr);
        }
    }
    let mut buf = [0; 1024];
    let res = timeout(
        Duration::from_millis(timeout_ms as u64),
        stream.read(&mut buf),
    )
    .await;
    match res {
        Ok(Ok(p)) => {
            known_packet(&buf[..p], packets, packet_queue).await;
            Ok(())
        }
        Err(t) => {
            trace!("Timeout: {:?}", t);
            // TODO: Retry sending the packet
            Err(SendError::Timeout)
        }
        Ok(Err(e)) => {
            trace!("Receive error: {:?}", e);
            Err(SendError::ReceiveErr)
        }
    }
}

/// This works by using the response packet as the key in a hashmap. If the packet is already in the hashmap we know that we have seen it before
async fn known_packet(
    response_packet: &[u8],
    input_packet: &Packets,
    packet_queue: &Arc<RwLock<PacketQueue>>,
) -> bool {
    // TODO: decode the packet and extract user id, payload, topic etc. because those don't matter to see if it is a known packet
    let queue_lock = packet_queue.read().await;
    let response_packet = response_packet.to_vec();
    if !queue_lock.inner.contains_key(&response_packet) {
        info!("New behavior discovered, adding it to the queue",);
        debug!("Response packet: {:?}", response_packet);
        drop(queue_lock);
        let mut queue_lock = packet_queue.write().await;
        queue_lock
            .inner
            .insert(response_packet, input_packet.clone());
    }
    trace!(
        "Known behavior. We have {} known behaviors",
        packet_queue.read().await.inner.len()
    );
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    use mqtt::packet::QoSWithPacketIdentifier;
    use mqtt::{Encodable, TopicFilter, TopicName};
    // To not generate these packets over and over again during execution of the markov model, we generate them here and then use them in the functions
    #[test]
    fn generate_connect_packet() {
        let mut connect = mqtt::packet::ConnectPacket::new("Hello MQTT Broker");
        connect.set_will(Some((
            TopicName::new("topic").unwrap(),
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 72, 255, 50, 0, 0, 0,
            ],
        )));
        let mut packet = Vec::new();
        connect.encode(&mut packet).unwrap();
        println!("{packet:?}");
    }

    #[test]
    fn generate_publish_packet() {
        let mut publish = mqtt::packet::PublishPacket::new(
            TopicName::new("topic").unwrap(),
            QoSWithPacketIdentifier::Level0,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 72, 255, 50, 0, 0, 0],
        );
        publish.set_retain(true);
        let mut packet = Vec::new();
        publish.encode(&mut packet).unwrap();
        println!("{packet:?}");
    }

    #[test]
    fn generate_subscribe_packet() {
        let subscribe = mqtt::packet::SubscribePacket::new(
            100,
            vec![(
                TopicFilter::new("topic").unwrap(),
                mqtt::QualityOfService::Level0,
            )],
        );
        let mut packet = Vec::new();
        subscribe.encode(&mut packet).unwrap();
        println!("{packet:?}");
    }

    #[test]
    fn generate_unsubscribe_packet() {
        let unsubscribe =
            mqtt::packet::UnsubscribePacket::new(10, vec![TopicFilter::new("topic").unwrap()]);
        let mut packet = Vec::new();
        unsubscribe.encode(&mut packet).unwrap();
        println!("{packet:?}");
    }

    #[test]
    fn generate_disconnect_packet() {
        let disconnect = mqtt::packet::DisconnectPacket::new();
        let mut packet = Vec::new();
        disconnect.encode(&mut packet).unwrap();
        println!("{packet:?}");
    }

    #[test]
    fn generate_pingreq_packet() {
        let pingreq = mqtt::packet::PingreqPacket::new();
        let mut packet = Vec::new();
        pingreq.encode(&mut packet).unwrap();
        println!("{packet:?}");
    }
}
