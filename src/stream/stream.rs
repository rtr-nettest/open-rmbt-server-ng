use anyhow::{Ok, Result};
use mio::{net::TcpStream, Interest, Poll, Token};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::Path;

use crate::client::constants::RMBT_UPGRADE_REQUEST;
use crate::stream::{
    websocket::WebSocketClient,
    websocket_tls_openssl::WebSocketTlsClient,
    openssl::OpenSslStream,
    rustls::RustlsStream,
    rustls_server::RustlsServerStream,
    websocket_rustls_server::WebSocketRustlsServerStream,
    
};
use crate::tokio_server::utils::websocket::Handshake;

#[derive(Debug)]
pub enum Stream {
    Tcp(TcpStream),
    WebSocket(WebSocketClient),
    OpenSsl(OpenSslStream),
    Rustls(RustlsStream),
    RustlsServer(RustlsServerStream),
    WebSocketTls(WebSocketTlsClient),
    WebSocketRustlsServer(WebSocketRustlsServerStream),
}

impl Stream {
    pub fn new_tcp(addr: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        Ok(Self::Tcp(stream))
    }

    pub fn return_type(&self) -> &str {
        match self {
            Stream::Tcp(_) => "Tcp",
            Stream::OpenSsl(_) => "OpenSsl",
            Stream::WebSocket(_) => "WebSocket",
            Stream::Rustls(_) => "Rustls",
            Stream::WebSocketTls(_) => "WebSocketTls",
            Stream::RustlsServer(_) => "RustlsServer",
            Stream::WebSocketRustlsServer(_) => "WebSocketRustlsServer",
        }
    }

    pub fn upgrade_to_websocket(self) -> Result<Stream> {
        match self {
            Stream::Tcp(stream) => {
                let stream = WebSocketClient::new_server(stream)?;
                Ok(Stream::WebSocket(stream))
            }
            Stream::RustlsServer(stream) => {
                let stream = WebSocketRustlsServerStream::from_rustls_server_stream(stream)?;
                Ok(Stream::WebSocketRustlsServer(stream))
            }
            _ => Ok(self),
        }
    }

    pub fn finish_server_handshake(&mut self, handshake: Handshake) -> Result<()> {
        match self {
            Stream::WebSocket(stream) => stream.finish_server_handshake(handshake),
            Stream::WebSocketRustlsServer(stream) => stream.finish_server_handshake(handshake),
            _ => Ok(()),
        }
    }




    pub fn new_websocket(addr: SocketAddr) -> Result<Self> {
        let ws_client = WebSocketClient::new(addr)?;
        Ok(Self::WebSocket(ws_client))
    }

    pub fn new_rustls(addr: SocketAddr, cert_path: Option<&Path>, key_path: Option<&Path>) -> Result<Self> {
        let stream = RustlsStream::new(addr, cert_path, key_path)?;
        Ok(Self::Rustls(stream))
    }

    pub fn new_rustls_server(stream: TcpStream, cert_path: String, key_path: String) -> Result<Self> {
        let stream = RustlsServerStream::new(stream, cert_path, key_path)?;
        Ok(Self::RustlsServer(stream))
    }

    pub fn close(&mut self) -> Result<()> {
        match self {
            Stream::Tcp(_) => Ok(()),
            Stream::OpenSsl(stream) => stream.close(),
            Stream::WebSocket(stream) => stream.close(),
            Stream::Rustls(_) => Ok(()),
            Stream::WebSocketTls(stream) => stream.close(),
            Stream::RustlsServer(_) => Ok(()),
            Stream::WebSocketRustlsServer(_) => Ok(()),
        }
    }

    pub fn get_greeting(&mut self) -> Vec<u8> {
        match self {
            Stream::Tcp(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
            Stream::OpenSsl(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
            Stream::WebSocket(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
            Stream::Rustls(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
            Stream::WebSocketTls(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
            Stream::RustlsServer(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
            Stream::WebSocketRustlsServer(_) => RMBT_UPGRADE_REQUEST.as_bytes().to_vec(),
        }
    }

    pub fn new_openssl(addr: SocketAddr) -> Result<Self> {
        let stream1 = TcpStream::connect(addr)?;
        stream1.set_nodelay(true)?;
        let stream = OpenSslStream::new(stream1, "localhost")?;
        Ok(Self::OpenSsl(stream))
    }
    

    pub fn new_websocket_tls(addr: SocketAddr) -> Result<Self> {
        let stream1 = TcpStream::connect(addr)?;
        stream1.set_nodelay(true)?;
        let stream = WebSocketTlsClient::new(addr,stream1, "localhost")?;
        Ok(Self::WebSocketTls(stream))
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Tcp(stream) => stream.read(buf),
            Stream::OpenSsl(stream) => stream.read(buf),
            Stream::WebSocket(stream) => stream.read(buf),
            Stream::Rustls(stream) => stream.read(buf),
            Stream::WebSocketTls(stream) => stream.read(buf),
            Stream::RustlsServer(stream) => stream.read(buf),
            Stream::WebSocketRustlsServer(stream) => stream.read(buf),
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Tcp(stream) => stream.write(buf),
            Stream::OpenSsl(stream) => stream.write(buf),
            Stream::WebSocket(stream) => stream.write(buf),
            Stream::Rustls(stream) => stream.write(buf),
            Stream::WebSocketTls(stream) => stream.write(buf),
            Stream::RustlsServer(stream) => stream.write(buf),
            Stream::WebSocketRustlsServer(stream) => stream.write(buf),
        }
    }

    pub fn register(&mut self, poll: &Poll, token: Token, interest: Interest) -> Result<()> {
        match self {
            Stream::Tcp(stream) => {
                poll.registry().register(stream, token, interest)?;
            }
            Stream::OpenSsl(stream) => {
                stream.register(poll, token, interest)?;
            }
            Stream::WebSocket(stream) => {
               stream.register(poll, token, interest)?;
            }
            Stream::Rustls(stream) => {
                stream.register(poll, token, interest)?;
            }
            Stream::WebSocketTls(stream) => {
                stream.register(poll, token, interest)?;
            }
            Stream::RustlsServer(stream) => {
                stream.register(poll, token, interest)?;
            }
            Stream::WebSocketRustlsServer(stream) => {
                stream.register(poll, token, interest)?;
            }
        }
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Tcp(stream) => stream.flush(),
            Stream::OpenSsl(stream) => stream.flush(),
            Stream::WebSocket(stream) => stream.flush(),
            Stream::Rustls(stream) => stream.flush(),
            Stream::WebSocketTls(stream) => stream.flush(),
            Stream::RustlsServer(stream) => stream.flush(),
            Stream::WebSocketRustlsServer(stream) => stream.flush(),
        }
    }

    pub fn reregister(&mut self, poll: &Poll, token: Token, interest: Interest) ->  io::Result<()>  {
        match self {
            Stream::Tcp(stream) => {
                poll.registry().reregister(stream, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            Stream::OpenSsl(stream) => {
                stream.reregister(poll, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            Stream::WebSocket(stream) => {
                stream.reregister(poll, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            Stream::Rustls(stream) => {
                stream.reregister(poll, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            Stream::WebSocketTls(stream) => {
                stream.reregister(poll, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            Stream::RustlsServer(stream) => {
                stream.reregister(poll, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            Stream::WebSocketRustlsServer(stream) => {
                stream.reregister(poll, token, interest)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
        }
    }
}
