use futures::{lock::Mutex, Future};
use mio::{net, Interest, Token};
use std::{
    io,
    net::SocketAddr,
    ops::Deref,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub fn create_tcpstream_connection(address: SocketAddr) -> io::Result<ConnectFuture> {
    let poll = mio::Poll::new()?;
    let events = mio::Events::with_capacity(128);
    let mut stream = net::TcpStream::connect(address)?;
    poll.registry()
        .register(&mut stream, Token(0), Interest::WRITABLE)?;
    Ok(ConnectFuture {
        poll,
        events: Arc::new(Mutex::new(events)),
        stream: Some(stream),
        is_ready: false,
    })
}
pub struct ConnectFuture {
    poll: mio::Poll,
    events: Arc<Mutex<mio::Events>>,
    stream: Option<net::TcpStream>,
    is_ready: bool,
}

impl Future for ConnectFuture {
    type Output = io::Result<net::TcpStream>;

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.is_ready {
            match self.events.clone().try_lock() {
                Some(mut events) => {
                    if let Err(err) = self
                        .poll
                        .poll(&mut *events, Some(std::time::Duration::from_millis(0)))
                    {
                        return Poll::Ready(Err(err));
                    }
                    match events
                        .deref()
                        .iter()
                        .find(|event| event.token() == Token(0) && event.is_writable())
                    {
                        Some(_) => {
                            self.is_ready = true;
                        }
                        None => {
                            return Poll::Pending;
                        }
                    }
                }
                None => {
                    return Poll::Pending;
                }
            }
        }

        let stream = self.stream.take().ok_or(io::Error::new(
            io::ErrorKind::Other,
            "Attempted to poll ConnectFuture after already connected",
        ))?;
        if let Ok(Some(err)) | Err(err) = stream.take_error() {
            return Poll::Ready(Err(err));
        }
        match stream.peer_addr() {
            Ok(_) => Poll::Ready(Ok(stream)),
            Err(err)
                if err.kind() == io::ErrorKind::NotConnected
                    || err.raw_os_error() == Some(libc::EINPROGRESS) =>
            {
                self.stream = Some(stream);
                self.is_ready = false;
                Poll::Pending
            }
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}
