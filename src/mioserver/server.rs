use crate::mioserver::control_server::auto_registration::{
    deregister_server, register_server, start_ping_job,
};
use bytes::BytesMut;
use log::{debug, info, LevelFilter};
use mio::net::{TcpListener, TcpStream};
use mio::Token;
use std::collections::VecDeque;
use std::io::{self};
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Instant;

#[derive(Debug)]
pub enum ConnectionType {
    Tcp(TcpStream, SocketAddr),
    Tls(TcpStream, SocketAddr), // Same TcpStream but with TLS flag
}

use crate::config::FileConfig;
use crate::mioserver::worker::WorkerThread;
use crate::mioserver::ServerTestPhase;
use crate::stream::stream::Stream;

pub struct MioServer {
    tcp_listeners: Vec<TcpListener>,
    tls_listeners: Vec<TcpListener>,
    _worker_threads: Vec<WorkerThread>,
    global_queue: Arc<Mutex<VecDeque<(ConnectionType, Instant)>>>, // Global queue with timestamps
    server_config: ServerConfig,
    shutdown_signal: Arc<AtomicBool>,
}

pub struct TestState {
    pub token: Token,
    pub connection_start: Instant,
    pub stream: Stream,
    pub measurement_state: ServerTestPhase,
    pub read_buffer: [u8; 1024 * 8],
    pub write_buffer: [u8; 1024 * 8],
    pub read_bytes: BytesMut,
    pub read_pos: usize,
    pub total_bytes_received: u64,
    pub total_bytes_sent: u64,
    pub write_pos: usize,
    pub num_chunks: usize,
    pub chunk_size: usize,
    pub processed_chunks: usize,
    pub clock: Option<Instant>,
    pub sent_time_ns: Option<u128>,
    pub received_time_ns: Option<u128>,
    pub duration: u64,
    pub put_duration: Option<u128>,
    pub chunk_buffer: Vec<u8>,
    pub loop_iteration_count: u32, // Counter for loop iterations
    pub chunk: Option<BytesMut>,
    pub terminal_chunk: Option<BytesMut>,
    pub bytes_received: VecDeque<(u64, u64)>,
    pub client_addr: Option<SocketAddr>,
    pub sig_key: Option<String>,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub tcp_address: Vec<SocketAddr>,
    pub tls_address: Vec<SocketAddr>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub num_workers: Option<usize>,
    pub user: Option<String>,
    pub daemon: bool,
    pub version: Option<String>,
    pub secret_key: String,
    pub log_level: Option<LevelFilter>,
    pub server_registration: bool,
    pub control_server: String,
    pub hostname: Option<String>,
    pub x_nettest_client: String,
    pub registration_token: Option<String>,
    pub server_name: Option<String>,
}

impl MioServer {
    pub fn new(args: Vec<String>, config: FileConfig) -> io::Result<Self> {
        let server_config = crate::mioserver::parser::parse_args(args, config.clone())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut tcp_listeners = Vec::new();
        for addr in &server_config.tcp_address {
            let tcp_listener = TcpListener::bind(*addr)?;
            debug!("MIO TCP Server will listen on {}", addr);
            tcp_listeners.push(tcp_listener);
        }

        let mut tls_listeners = Vec::new();
        if server_config.cert_path.is_some() && server_config.key_path.is_some() {
            for addr in &server_config.tls_address {
                let tls_listener = TcpListener::bind(*addr)?;
                debug!("MIO TLS Server will listen on {}", addr);
                tls_listeners.push(tls_listener);
            }
        }
        
        let logical = server_config.num_workers.unwrap_or(30);

        let worker_connection_counts = Arc::new(Mutex::new(vec![0; logical]));
        let global_queue = Arc::new(Mutex::new(VecDeque::new()));

        let mut worker_threads = Vec::new();

        for i in 0..logical {
            let worker = WorkerThread::new(
                i,
                worker_connection_counts.clone(),
                global_queue.clone(),
                server_config.clone(),
            )?;
            worker_threads.push(worker);
        }

        Ok(Self {
            tcp_listeners: tcp_listeners,
            tls_listeners: tls_listeners,
            _worker_threads: worker_threads,
            global_queue,
            server_config,
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
        info!(
            "server_config.server_registration: {:?}",
            self.server_config.server_registration
        );
        if self.server_config.server_registration {
            info!("Registering server with control server...");
            let config_clone = self.server_config.clone();
            info!("Registering server with control server...");
            let shutdown_signal = self.shutdown_signal.clone();
            tokio::spawn(async move {
                match register_server(&config_clone).await {
                    Ok(_) => {
                        info!("Server registration successful, starting ping job...");
                        start_ping_job(config_clone, shutdown_signal).await;
                    }
                    Err(e) => {
                        info!("Server registration failed: {}", e);
                    }
                }
            });
        }

        loop {
            // Check shutdown signal
            if self.shutdown_signal.load(Ordering::Relaxed) {
                info!("Shutdown signal received, stopping server...");
                break;
            }

            // Accept TCP connections
            let mut tcp_connections = Vec::new();
            for tcp_listener in &self.tcp_listeners {
                match tcp_listener.accept() {
                    Ok((stream, addr)) => {
                        if let Err(e) = stream.set_nodelay(true) {
                            debug!("Failed to set TCP_NODELAY: {}", e);
                        }
                        tcp_connections.push((stream, addr));
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // Continue
                    }
                    Err(e) => {
                        debug!("Error accepting TCP connection: {}", e);
                        return Err(e);
                    }
                }
            }

            // Process accepted TCP connections
            for (stream, addr) in tcp_connections {
                self.handle_connection(stream, false, addr)?;
            }

            // Accept TLS connections if listener exists
            let mut tls_connections = Vec::new();
            for tls_listener in &self.tls_listeners {
                match tls_listener.accept() {
                    Ok((stream, addr)) => {
                        if let Err(e) = stream.set_nodelay(true) {
                            debug!("Failed to set TCP_NODELAY: {}", e);
                        }
                        tls_connections.push((stream, addr));
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // Continue
                    }
                    Err(e) => {
                        debug!("Error accepting TLS connection: {}", e);
                        return Err(e);
                    }
                }
            }

            // Process accepted TLS connections
            for (stream, addr) in tls_connections {
                self.handle_connection(stream, true, addr)?;
            }

            // Check global queue for stale connections
            self.check_global_queue()?;

            thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
    }

    pub async fn shutdown(&mut self) -> io::Result<()> {
        info!("Starting graceful shutdown...");

        if self.server_config.server_registration {
            info!("Deregistering server from control server...");
            let _ = deregister_server(&self.server_config).await;
        }

        info!("Server shutdown complete");
        Ok(())
    }

    pub fn request_shutdown(&self) {
        self.shutdown_signal.store(true, Ordering::Relaxed);
        info!("Shutdown requested");
    }

    pub fn get_shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
    }

    fn check_global_queue(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn handle_connection(
        &mut self,
        stream: TcpStream,
        is_tls: bool,
        client_addr: SocketAddr,
    ) -> io::Result<()> {
        let connection = if is_tls {
            ConnectionType::Tls(stream, client_addr)
        } else {
            ConnectionType::Tcp(stream, client_addr)
        };

        // Add connection to global queue
        let mut global_queue = self.global_queue.lock().unwrap();
        global_queue.push_back((connection, Instant::now()));

        info!(
            "{} connection added to global queue (queue size: {})",
            if is_tls { "TLS" } else { "TCP" },
            global_queue.len()
        );

        Ok(())
    }
}

impl Drop for MioServer {
    fn drop(&mut self) {
        debug!("MIO TCP Server shutting down");
    }
}
