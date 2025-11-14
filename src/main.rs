use log::{debug, info};
use tokio::signal;

use crate::config::parser::read_config_file;
use crate::mioserver::MioServer;
use std::error::Error as StdError;

pub mod config;
pub mod logger;
pub mod mioserver;
pub mod stream;
pub mod tokio_server;

pub mod client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError + Send + Sync>> {
    let mut args: Vec<String> = std::env::args().collect();

    let config_result = read_config_file();
    if config_result.is_err() {
        return Err(config_result.err().unwrap().into());
    }
    let config = config_result.unwrap();
    if args.len() == 1 || args[1] == "-c" {
        args = args.iter().skip(1).map(|s| s.clone()).collect();
        client::client::client_run(args, config).await?;
        return Ok(());
    } else if args[1] == "-s" {
        debug!("args: {:?}", args);
        args = args.iter().skip(1).map(|s| s.clone()).collect();

        let mut mio_server = MioServer::new(args, config)?;

        // Create separate thread for signal handling
        let shutdown_signal = mio_server.get_shutdown_signal();
        tokio::spawn(async move {
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            info!("Ctrl+C received, shutting down server...");
            shutdown_signal.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        mio_server.run()?;
        info!("Server stopping...");
        mio_server.shutdown().await?;
        info!("Server stopped");
    } else {
        if args[1] != "-h" && args[1] != "--help" {
            println!("Invalid initial arguments");
        }
        println!("Usage: nettest -s [options] or nettest -c [options]");
        println!("Run ./nettest without arguments to automatically find nearest server and run measurement against it");
        println!("For more information, use 'nettest -c -h' or 'nettest -s -h'");
    }
    Ok(())
}
