use std::{
    io::Result,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::AsyncRead,
    net::{TcpStream, UnixStream},
};

pub enum DaemonStream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl AsyncRead for DaemonStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        match &mut *self {
            DaemonStream::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            DaemonStream::Unix(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}
