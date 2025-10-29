use anyhow::{Context, Result};
use std::{pin::Pin, task::Poll};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, UnixStream},
};

use crate::network::SocketAddr;

/// Unified stream abstraction for both TCP and Unix sockets.
#[derive(Debug)]
pub enum Stream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Stream::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            Stream::Unix(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    // Required methods
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Stream::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            Stream::Unix(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Stream::Tcp(s) => Pin::new(s).poll_flush(cx),
            Stream::Unix(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Stream::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            Stream::Unix(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

impl Stream {
    pub async fn connect(sock: SocketAddr) -> Result<Stream> {
        Ok(match sock {
            SocketAddr::Tcp(addr) => Self::Tcp(
                TcpStream::connect(addr)
                    .await
                    .context("Failed to connect over TCP")?,
            ),
            SocketAddr::Unix(addr) => Self::Unix(
                UnixStream::connect(addr.context("Can't connect to an unnamed socket.")?)
                    .await
                    .context("Failed to connect over Unix")?,
            ),
        })
    }

    pub fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(match self {
            Stream::Tcp(stream) => SocketAddr::Tcp(
                stream
                    .peer_addr()
                    .context("Failed to fetch peer address from TCP.")?,
            ),
            Stream::Unix(stream) => SocketAddr::from(
                stream
                    .peer_addr()
                    .context("Failed to fetch peer address from Unix.")?,
            ),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(match self {
            Stream::Tcp(stream) => SocketAddr::Tcp(
                stream
                    .local_addr()
                    .context("Failed to fetch local address from TCP.")?,
            ),
            Stream::Unix(stream) => SocketAddr::from(
                stream
                    .local_addr()
                    .context("Failed to fetch local address from Unix.")?,
            ),
        })
    }
}
