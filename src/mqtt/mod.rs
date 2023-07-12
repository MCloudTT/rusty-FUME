use crate::markov::ByteStream;
use rand::prelude::ThreadRng;
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

pub(crate) fn generate_connect_packet(rng: &mut ThreadRng) -> Vec<u8> {
    todo!()
}

pub(crate) async fn test_connection(stream: &mut impl ByteStream) -> color_eyre::Result<()> {
    let packets = CONNECT.split('\n').map(|x| hex::decode(x).unwrap());
    for packet in packets {
        stream.write_all(&packet).await?;
        let mut buf = [0; 1024];
        let _ = timeout(Duration::from_secs(2), stream.read(&mut buf)).await;
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
