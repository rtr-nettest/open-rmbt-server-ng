use anyhow::{Error, Result};
use log::{debug, info, trace};
use mio::{net::TcpStream, Interest, Poll, Token};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use std::fs;
use std::io::{self, BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;


#[derive(Debug)]
pub struct RustlsStream {
    pub conn: ClientConnection,
    pub stream: TcpStream,
    handshake_done: bool,
    pub finished: bool,
    pub temp_buf: Vec<u8>,
}

impl RustlsStream {
    pub fn new(
        addr: SocketAddr,
        cert_path: Option<&Path>,
        key_path: Option<&Path>,
    ) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;

        let config = if let (Some(cert_path), Some(key_path)) = (cert_path, key_path) {
            let mut root_store = RootCertStore::empty();
            let certs = load_certs(cert_path)?;

            for cert in &certs {
                root_store.add(cert.clone())?;
            }

            let key = load_private_key(key_path)?;

            let config = ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_client_auth_cert(certs, key)?;
            config
        } else {
            let config = ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(danger::NoCertificateVerification))
                .with_no_client_auth();
            config
        };

        // Используем IP-адрес как домен для локального соединения
        let server_name = if addr.ip().is_loopback() {
            ServerName::DnsName("localhost".try_into()?)
        } else {
            ServerName::DnsName("dev.measurementservers.net".try_into()?)
        };

        let  conn = ClientConnection::new(Arc::new(config), server_name)?;

        // conn.set_buffer_limit(Some(1024 * 1024 * 10));

        Ok(Self {
            conn,
            stream,
            handshake_done: false,
            finished: true,
            temp_buf: Vec::new(),
        })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // trace!("Reading from RustlsStream");
        if self.temp_buf.len() > buf.len() {

            let to_copy = buf.len().min(self.temp_buf.len());
            buf[..to_copy].copy_from_slice(&self.temp_buf[..to_copy]);
            self.temp_buf.drain(..to_copy);
            debug!("Left {} bytes in temp_buf", self.temp_buf.len());
            return Ok(to_copy);
        } 



        // Read TLS data
        match self.conn.read_tls(&mut self.stream) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // trace!("TLS read would block");
                // Принудительно отправляем данные для очистки буфера
                if self.conn.wants_write() {
                    self.conn.write_tls(&mut self.stream)?;
                }
                debug!("WouldBlock 1");
                if self.temp_buf.len() > 0 {
                    let to_copy = buf.len().min(self.temp_buf.len());
                    buf[..to_copy].copy_from_slice(&self.temp_buf[..to_copy]);
                    self.temp_buf.drain(..to_copy);
                    debug!("Left {} bytes in temp_buf", self.temp_buf.len());
                    return Ok(to_copy);
                }  
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "TLS read would block",
                ));
            }
            Err(e) => {
                debug!("TLS read error: {:?}", e);
                return Err(e);
            }
            Ok(0) => {
                // trace!("TLS connection closed");
                return Ok(0);
            }
            Ok(n) => {
                trace!("Read  {} bytes from TLS", n);
            }
        }

        trace!("Temp buf len: {}", self.temp_buf.len());

        // Process any new TLS messages
        // debug!("Processing new packets");
        let io_state = match self.conn.process_new_packets() {
            Ok(state) => {
                debug!("Processed new packets {:?}", state.plaintext_bytes_to_read());
                state
            }
            Err(e) => {
                trace!("Error processing new packets: {:?}", e);
                return Err(io::Error::new(io::ErrorKind::Other, e));
            }
        };


    

        // If we need to write handshake data, do it
        if !self.handshake_done {
            while self.conn.wants_write() {
                self.handshake_done = true;
                trace!("Writing handshake data");
                self.conn.write_tls(&mut self.stream)?;
            }
            self.handshake_done = true;
        }


        
        let mut chunk = [0u8; 8192 * 10];
            

        loop {
            match self.conn.reader().read(&mut chunk) {
                Ok(0) => {
                    trace!("EOF");
                    break;
                },
                Ok(n) => {
                    self.temp_buf.extend_from_slice(&chunk[..n]);
                    trace!("Extending temp buf by {} bytes", n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    trace!("WouldBlock nothing to read from conn");
                    break;
                },
                Err(e) => return Err(e),
            }
        }

        let to_copy = buf.len().min(self.temp_buf.len());

        buf[..to_copy].copy_from_slice(&self.temp_buf[..to_copy]);

        trace!("To copy: {} from temp buf len: {}", to_copy, self.temp_buf.len());
        if to_copy > 0 {
            self.temp_buf.drain(..to_copy);
            trace!("Temp buf len after drain : {}", self.temp_buf.len());
            return Ok(to_copy);
        }

        trace!("No data to copy");
        // If we need to read more data, try again
        if self.conn.wants_read() {
            trace!("Wants read");
            return self.read(buf);
        }

        // Принудительно отправляем данные для очистки буфера
        if self.conn.wants_write() {
            trace!("Wants write");
            self.conn.write_tls(&mut self.stream)?;
        }

        // Check if peer has closed
        if io_state.peer_has_closed() {
            trace!("Peer has closed the connection");
            return Ok(0);
        }

        Ok(0)
    }

    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut total_written = 0;

        // Проверяем, есть ли данные в буфере для отправки
        while self.conn.wants_write() {
            match self.conn.write_tls(&mut self.stream) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    trace!("TLS buffer flush would block");
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "TLS write would block",
                    ));
                }
                Err(e) => {
                    info!("TLS buffer flush error: {:?}", e);
                    return Err(e);
                }
            }
        }

        if !self.finished {
            total_written += 1;
            self.finished = true;
        }


        // Теперь пробуем записать новые данные
        while total_written < buf.len() {
            match self.conn.writer().write(&buf[total_written..]) {
                Ok(n) => {
                    total_written += n;

                    // Пытаемся отправить данные в сеть
                    while self.conn.wants_write() {
                        match self.conn.write_tls(&mut self.stream) {
                            Ok(_) => {
                                continue;
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                self.finished = false;
                                return Ok(total_written - 1);
                            }
                            Err(e) => {
                                debug!("Network write error: {:?}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    debug!("TLS buffer write would block");
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "TLS write would block",
                    ));
                }
                Err(e) => {
                    debug!("TLS buffer write error: {:?}", e);
                    return Err(e);
                }
            }
        }


        Ok(total_written)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }

    pub fn register(&mut self, poll: &Poll, token: Token, interests: Interest) -> io::Result<()> {
        poll.registry().register(&mut self.stream, token, interests)
    }

    pub fn reregister(&mut self, poll: &Poll, token: Token, interests: Interest) -> io::Result<()> {
        poll.registry()
            .reregister(&mut self.stream, token, interests)
    }

    pub fn perform_handshake(&mut self) -> Result<()> {
        info!("TLS handshake completed successfully");
        Ok(())
    }

    pub fn close(&mut self) -> io::Result<()> {
        debug!("Closing TLS connection");

        // Отправляем close_notify
        if self.conn.wants_write() {
            match self.conn.write_tls(&mut self.stream) {
                Ok(_) => debug!("Sent TLS close_notify"),
                Err(e) => debug!("Error sending TLS close_notify: {:?}", e),
            }
        }

        // Закрываем TCP соединение
        self.stream.shutdown(std::net::Shutdown::Both)
    }
}

fn load_certs(cert_path: &Path) -> Result<Vec<CertificateDer<'static>>, Error> {
    let cert_path = cert_path.to_str().unwrap();
    debug!("Loading certificates from {}", cert_path);
    let certfile = fs::read(cert_path)?;
    debug!("Read {} bytes from certificate file", certfile.len());
    let mut reader = BufReader::new(certfile.as_slice());
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    debug!("Successfully parsed {} certificates", certs.len());
    Ok(certs)
}

fn load_private_key(key_path: &Path) -> Result<PrivateKeyDer<'static>, Error> {
    let key_path = key_path.to_str().unwrap();
    debug!("Loading private key from {}", key_path);
    let keyfile = fs::read(key_path)?;
    debug!("Read {} bytes from key file", keyfile.len());
    let mut reader = BufReader::new(keyfile.as_slice());

    // Try to read any private key format
    if let Some(key) = rustls_pemfile::private_key(&mut reader)? {
        debug!("Successfully loaded private key: {:?}", key);
        Ok(key)
    } else {
        debug!("No private keys found in key file");
        Err(Error::msg("No private keys found in key file"))
    }
}

mod danger {
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::UnixTime;
    use rustls::pki_types::{CertificateDer, ServerName};
    use rustls::DigitallySignedStruct;

    #[derive(Debug)]
    pub struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer,
            _intermediates: &[CertificateDer],
            _server_name: &ServerName,
            _scts: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            vec![
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA384,
                rustls::SignatureScheme::RSA_PSS_SHA512,
                rustls::SignatureScheme::RSA_PKCS1_SHA256,
                rustls::SignatureScheme::RSA_PKCS1_SHA384,
                rustls::SignatureScheme::RSA_PKCS1_SHA512,
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
                rustls::SignatureScheme::ED25519,
            ]
        }
    }
}
