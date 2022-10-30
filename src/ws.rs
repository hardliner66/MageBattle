use crate::tcpstream::{create_tcpstream_connection, ConnectFuture};
use anyhow::anyhow;
use async_recursion::async_recursion;
use futures::future;
use macroquad::prelude::coroutines::wait_seconds;
use mio::net;
use std::{io, net::ToSocketAddrs, sync::Mutex};
use tungstenite::{
    client::{client, IntoClientRequest},
    handshake::MidHandshake,
    ClientHandshake, HandshakeError, Message, WebSocket,
};

#[derive(Default)]
pub struct Connection {
    socket: Mutex<Option<WebSocket<net::TcpStream>>>,
}

impl Connection {
    pub fn new() -> Self {
        Self::default()
    }

    #[async_recursion]
    #[allow(clippy::similar_names)]
    pub async fn connect(&self, url: &str) -> anyhow::Result<()> {
        let req = url.into_client_request()?;
        let uri = req.uri().clone();
        let host = uri.host().ok_or(tungstenite::Error::Url(
            tungstenite::error::UrlError::NoHostName,
        ))?;
        let port = uri.port_u16().unwrap_or(80);
        let addresses = (host, port).to_socket_addrs()?;
        let stream_futures = addresses
            .map(create_tcpstream_connection)
            .collect::<io::Result<Vec<ConnectFuture>>>()?;
        if let Err(err) = self.connect_internal(stream_futures, url).await {
            log::error!(
                "Failed to connect to {}, attempting again in 1 second: {}",
                url,
                err
            );
            wait_seconds(1.0).await;
            self.connect(url).await?;
        }
        log::info!("Connection established successfully");
        Ok(())
    }

    pub fn restart(&self) {
        let mut socket_lock = self.socket.lock().unwrap();
        *socket_lock = None;
    }

    async fn connect_internal(
        &self,
        connect_futures: Vec<ConnectFuture>,
        url: &str,
    ) -> anyhow::Result<()> {
        let streams = future::join_all(connect_futures).await;
        let stream = streams
            .into_iter()
            .find_map(std::result::Result::ok)
            .ok_or_else(|| anyhow!("Failed to connect to {}", url))?;
        let socket = match client(url, stream) {
            Ok((socket, _)) => Ok(socket),
            Err(err) => {
                if let HandshakeError::Interrupted(mid_handshake) = err {
                    Self::retry_handshake(mid_handshake)
                } else {
                    Err(err)
                }
            }
        }?;
        let mut socket_lock = self.socket.lock().map_err(|err| anyhow!("{}", err))?;
        *socket_lock = Some(socket);
        Ok(())
    }

    fn retry_handshake(
        mid_handshake: MidHandshake<ClientHandshake<net::TcpStream>>,
    ) -> tungstenite::Result<
        WebSocket<net::TcpStream>,
        HandshakeError<ClientHandshake<net::TcpStream>>,
    > {
        match mid_handshake.handshake() {
            Ok((socket, _)) => Ok(socket),
            Err(err) => match err {
                HandshakeError::Interrupted(mid_handshake) => Self::retry_handshake(mid_handshake),
                HandshakeError::Failure(_) => Err(err),
            },
        }
    }

    pub fn poll(&self) -> Option<Vec<u8>> {
        if let Ok(mut socket_lock) = self.socket.try_lock() {
            if let Some(socket) = socket_lock.as_mut() {
                if let Ok(Message::Binary(msg)) = socket.read_message() {
                    return Some(msg);
                }
            }
        }
        None
    }

    pub fn send(&self, msg: Vec<u8>) -> Result<(), tungstenite::Error> {
        if let Ok(mut socket_lock) = self.socket.try_lock() {
            let socket = socket_lock.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotConnected, "No socket connection")
            })?;
            socket.write_message(Message::Binary(msg))?;
        }
        Ok(())
    }
}
