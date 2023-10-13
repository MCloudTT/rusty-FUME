use color_eyre::Result;
// Avoid double protocols
// We have a lot of features which are mutually exclusive, so we need compile time errors if someone tries to use them together.
// These are the features: tcp, websocket, tls, quic
#[cfg(all(
    feature = "tcp",
    any(feature = "websocket", feature = "tls", feature = "quic")
))]
compile_error!(
    "You can only use one protocol at a time. Please disable tcp with --no-default-features"
);
#[cfg(all(feature = "websocket", any(feature = "tls", feature = "quic")))]
compile_error!(
    "You can only use one protocol at a time. Please disable websocket with --no-default-features"
);
#[cfg(all(feature = "tls", feature = "quic"))]
compile_error!(
    "You can only use one protocol at a time. Please disable tls with --no-default-features"
);

#[cfg(feature = "tcp")]
pub use tcp::*;
#[cfg(feature = "tcp")]
mod tcp {
    use super::*;
    use std::time::Duration;
    use tokio::net::{TcpStream, ToSocketAddrs};

    pub async fn connect_to_broker(address: impl ToSocketAddrs, timeout: u16) -> Result<TcpStream> {
        let tcp_stream = tokio::time::timeout(
            Duration::from_millis(timeout as u64),
            TcpStream::connect(address),
        )
        .await;
        Ok(tcp_stream??)
    }
}

#[cfg(feature = "websocket")]
pub use websocket::*;

#[cfg(feature = "websocket")]
mod websocket {
    compile_error!("Websocket is not implemented yet");
    /*
    use super::*;
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

    pub async fn connect_to_broker(
        address: impl IntoClientRequest + Unpin,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(address).await?;
        Ok(ws_stream)
    }
     */
}

#[cfg(feature = "tls")]
pub use tls::*;

#[cfg(feature = "tls")]
mod tls {
    use super::*;
    use crate::markov::ByteStream;
    use futures::FutureExt;
    use rustls::client::{ServerCertVerified, ServerCertVerifier};
    use rustls::{Certificate, ClientConfig, ClientConnection, Error, ServerName};
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::SystemTime;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpStream, ToSocketAddrs};
    use tokio_rustls::client::TlsStream;
    use tokio_rustls::TlsConnector;

    struct NoCertificateVerifier;

    impl ServerCertVerifier for NoCertificateVerifier {
        fn verify_server_cert(
            &self,
            end_entity: &Certificate,
            intermediates: &[Certificate],
            server_name: &ServerName,
            scts: &mut dyn Iterator<Item = &[u8]>,
            ocsp_response: &[u8],
            now: SystemTime,
        ) -> std::result::Result<ServerCertVerified, Error> {
            Ok(ServerCertVerified::assertion())
        }
    }
    /// Connects to the broker using TLS ignoring the server certificate since that would just be
    /// wasted iterations
    pub async fn connect_to_broker(address: &str) -> Result<TlsStream<TcpStream>> {
        let mut socket = TcpStream::connect(address).await?;
        let mut config = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(NoCertificateVerifier))
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let stream = connector
            .connect(
                // Trim everything including the port beginning ':'
                ServerName::IpAddress(
                    IpAddr::from_str(address.split(':').next().unwrap()).unwrap(),
                ),
                socket,
            )
            .await?;
        Ok(stream)
    }
}
