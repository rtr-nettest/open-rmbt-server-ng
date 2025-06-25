use std::time::Duration;

use futures::{SinkExt, StreamExt};
use log::{debug, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_rustls::client::TlsStream;
use tokio_tungstenite::WebSocketStream;
use std::error::Error;

const CHUNK_SIZE: usize = 4096;

#[derive(Debug)]
pub enum Stream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
    WebSocket(WebSocketStream<TcpStream>),
    WebSocketTls(WebSocketStream<TlsStream<TcpStream>>),
}

impl Stream {
    pub async fn upgrade_to_websocket(self) -> std::io::Result<Stream> {
        match self {
            Stream::Plain(mut tcp_stream) => {
                debug!("Attempting to upgrade plain TCP stream to WebSocket");
                
                // Ручной WebSocket handshake
                let handshake_request = "GET / HTTP/1.1\r\n\
                    Host: localhost\r\n\
                    Connection: Upgrade\r\n\
                    Upgrade: websocket\r\n\
                    Sec-WebSocket-Version: 13\r\n\
                    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                    \r\n";
                
                // Отправляем handshake request
                tcp_stream.write_all(handshake_request.as_bytes()).await?;
                
                // Читаем response
                let mut response = [0u8; 1024];
                let n = tcp_stream.read(&mut response).await?;
                let response_str = String::from_utf8_lossy(&response[..n]);
                
                if !response_str.contains("HTTP/1.1 101") {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid WebSocket handshake response",
                    ));
                }
                
                // Создаем WebSocket stream
                let ws_stream = WebSocketStream::from_raw_socket(
                    tcp_stream,
                    tokio_tungstenite::tungstenite::protocol::Role::Client,
                    None,
                ).await;
                
                debug!("Successfully upgraded plain TCP stream to WebSocket");
                Ok(Stream::WebSocket(ws_stream))
            }
            Stream::Tls(mut tls_stream) => {
                debug!("Attempting to upgrade TLS stream to WebSocket");
                
                // Ручной WebSocket handshake поверх TLS
                let handshake_request = "GET / HTTP/1.1\r\n\
                    Host: localhost\r\n\
                    Connection: Upgrade\r\n\
                    Upgrade: websocket\r\n\
                    Sec-WebSocket-Version: 13\r\n\
                    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                    \r\n";
                
                // Отправляем handshake request
                tls_stream.write_all(handshake_request.as_bytes()).await?;
                
                // Читаем response
                let mut response = [0u8; 1024];
                let n = tls_stream.read(&mut response).await?;
                let response_str = String::from_utf8_lossy(&response[..n]);
                
                if !response_str.contains("HTTP/1.1 101") {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid WebSocket handshake response",
                    ));
                }
                
                // Создаем WebSocket stream
                let ws_stream = WebSocketStream::from_raw_socket(
                    tls_stream,
                    tokio_tungstenite::tungstenite::protocol::Role::Client,
                    None,
                ).await;
                
                debug!("Successfully upgraded TLS stream to WebSocket");
                Ok(Stream::WebSocketTls(ws_stream))
            }
            _ => {
                debug!("Cannot upgrade non-TCP stream to WebSocket");
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot upgrade non-TCP stream to WebSocket",
                ))
            }
        }
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // sleep(Duration::from_millis(50)).await;
        match self {
            Stream::Plain(stream) => stream.read(buf).await,
            Stream::Tls(stream) => stream.read(buf).await,
            Stream::WebSocket(stream) => {
                if let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => {
                            let data = match msg {
                                tokio_tungstenite::tungstenite::Message::Binary(data) => data,
                                tokio_tungstenite::tungstenite::Message::Text(text) => {
                                    text.into_bytes()
                                }
                                _ => return Ok(0),
                            };
                            let len = data.len().min(buf.len());
                            buf[..len].copy_from_slice(&data[..len]);
                            Ok(len)
                        }
                        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
                    }
                } else {
                    Ok(0)
                }
            }
            Stream::WebSocketTls(stream) => {
                if let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => {
                            let data = match msg {
                                tokio_tungstenite::tungstenite::Message::Binary(data) => data,
                                tokio_tungstenite::tungstenite::Message::Text(text) => {
                                    text.into_bytes()
                                }
                                _ => return Ok(0),
                            };
                            let len = data.len().min(buf.len());
                            buf[..len].copy_from_slice(&data[..len]);
                            Ok(len)
                        }
                        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
                    }
                } else {
                    Ok(0)
                }
            }
        }
    }

    pub async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(stream) => {
                // info!("Writing to plain stream");
                // info!("Writing to plain stream: {:?}", String::from_utf8_lossy(buf));

                stream.write(buf).await
            },
            Stream::Tls(stream) => stream.write(buf).await,
            Stream::WebSocket(stream) => {
                let message = if buf.len() < 2 || buf.len() > (CHUNK_SIZE - 3) {
                    tokio_tungstenite::tungstenite::Message::Binary(buf.to_vec())
                } else {
                    tokio_tungstenite::tungstenite::Message::Text(
                        String::from_utf8_lossy(buf).to_string(),
                    )
                };
                stream.send(message).await.map_err(|e| {
                    debug!("WebSocket: Error sending message in write: {}", e);
                    std::io::Error::new(std::io::ErrorKind::Other, e)
                })?;
                Ok(buf.len())
            }
            Stream::WebSocketTls(stream) => {
                let message = if buf.len() < 2 || buf.len() > (CHUNK_SIZE - 3) {
                    tokio_tungstenite::tungstenite::Message::Binary(buf.to_vec())
                } else {
                    tokio_tungstenite::tungstenite::Message::Text(
                        String::from_utf8_lossy(buf).to_string(),
                    )
                };
                stream.send(message).await.map_err(|e| {
                    debug!("WebSocketTls: Error sending message in write: {}", e);
                    std::io::Error::new(std::io::ErrorKind::Other, e)
                })?;
                Ok(buf.len())
            }
        }
    }

    pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            Stream::Plain(stream) => {
                // info!("Writing to plain stream {} last byte: {}", buf.len(), buf[buf.len() - 1]);
                // info!("Writing to plain stream: {:?}", String::from_utf8_lossy(buf));
                // sleep(Duration::from_millis(20)).await;
                stream.write_all(buf).await
            },
            Stream::Tls(stream) => {
                // info!("Writing to TLS stream");
                stream.write_all(buf).await
            },
            Stream::WebSocket(stream) => {
                let message = if buf.len() < 2 || buf.len() > (CHUNK_SIZE - 3) {
                    tokio_tungstenite::tungstenite::Message::Binary(buf.to_vec())
                } else {
                    tokio_tungstenite::tungstenite::Message::Text(
                        String::from_utf8_lossy(buf).to_string(),
                    )
                };
                stream.send(message).await.map_err(|e| {
                    debug!("WebSocket: Error sending message: {}", e);
                    std::io::Error::new(std::io::ErrorKind::Other, e)
                })?;
                Ok(())
            }
            Stream::WebSocketTls(stream) => {
                let message = if buf.len() < 2 || buf.len() > (CHUNK_SIZE - 3) {
                    tokio_tungstenite::tungstenite::Message::Binary(buf.to_vec())
                } else {
                    tokio_tungstenite::tungstenite::Message::Text(
                        String::from_utf8_lossy(buf).to_string(),
                    )
                };
                stream.send(message).await.map_err(|e| {
                    debug!("WebSocketTls: Error sending message: {}", e);
                    std::io::Error::new(std::io::ErrorKind::Other, e)
                })?;
                Ok(())
            }
        }
    }

    pub async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(stream) => stream.flush().await,
            Stream::Tls(stream) => stream.flush().await,
            Stream::WebSocket(_) => Ok(()),
            Stream::WebSocketTls(_) => Ok(()),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Stream::Plain(_) => "Plain".to_string(),
            Stream::Tls(_) => "TLS".to_string(),
            Stream::WebSocket(_) => "WebSocket".to_string(),
            Stream::WebSocketTls(_) => "WebSocketTLS".to_string(),
        }
    }
}
