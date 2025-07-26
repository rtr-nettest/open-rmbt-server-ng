use crate::logger;
use crate::tokio_server::utils::{daemon, user};
use log::{debug, LevelFilter};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

#[derive(Debug, Serialize, Deserialize)]
pub struct RmbtServerConfig {
    pub listen_addresses: Vec<SocketAddr>,
    pub ssl_listen_addresses: Vec<SocketAddr>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub num_threads: usize,
    pub user: Option<String>,
    pub daemon: bool,
    pub user_privileges: bool,
    pub debug: bool,
    pub websocket: bool,
    pub version: Option<u8>,
    pub secret_keys: Vec<String>,
    pub secret_key_labels: Vec<String>,
}


impl RmbtServerConfig {
    pub fn from_args(args: Vec<String>) -> Result<Self, Box<dyn Error + Send + Sync>> {

        // if args.len() == 1 {
        //     let addr = "127.0.0.1:5005".parse().unwrap();
        //     let config = RmbtServerConfig {
        //         listen_addresses: vec![addr],
        //         ssl_listen_addresses: Vec::new(),
        //         user_privileges: false,
        //         cert_path: None,
        //         key_path: None,
        //         num_threads: 200, // Default value from C version
        //         user: None,
        //         daemon: false,
        //         debug: false,
        //         websocket: false,
        //         version: None,
        //         secret_keys: Vec::new(),
        //         secret_key_labels: Vec::new(),
        //     };
        //     logger::init_logger(LevelFilter::Info)?;
        //     return Ok(config);
        // }
        // Show help if no arguments or help flag is present
        if  args.contains(&"-h".to_string())
            || args.contains(&"--help".to_string())
        {
            print_help();
            return Err("Help printed".into());
        }

      

        Self::from_args_vec(args)
    }

    pub fn from_args_vec(args: Vec<String>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut config = RmbtServerConfig {
            listen_addresses: Vec::new(),
            ssl_listen_addresses: Vec::new(),
            user_privileges: false,
            cert_path: None,
            key_path: None,
            num_threads: 200, // Default value from C version
            user: None,
            daemon: false,
            debug: false,
            websocket: false,
            version: None,
            secret_keys: Vec::new(),
            secret_key_labels: Vec::new(),
        };

        // // Try to read secret keys from file
        // match secret_keys::read_secret_keys("secret.key") {
        //     Ok(keys) => {
        //         config.secret_keys = keys.iter().map(|k| k.key.clone()).collect();
        //         config.secret_key_labels = keys.iter().map(|k| k.label.clone()).collect();
        //     }
        //     Err(e) => {
        //         // error!("Error while opening secret.key: {}", e);
        //         return Err(format!("Error while opening secret.key: {}", e).into());
        //     }
        // }

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-l" | "-L" => {
                    i += 1;
                    if i < args.len() {
                        let addr = parse_listen_address(&args[i])?;
                        if args[i - 1] == "-L" {
                            config.ssl_listen_addresses.push(addr);
                        } else {
                            config.listen_addresses.push(addr);
                        }
                    }
                }
                "-c" => {
                    i += 1;
                    if i < args.len() {
                        if config.cert_path.is_some() {
                            return Err("Error: only one -c is allowed".into());
                        }
                        config.cert_path = Some(args[i].clone());
                    }
                }
                "-k" => {
                    i += 1;
                    if i < args.len() {
                        if config.key_path.is_some() {
                            return Err("Error: only one -k is allowed".into());
                        }
                        config.key_path = Some(args[i].clone());
                    }
                }
                "-t" => {
                    i += 1;
                    if i < args.len() {
                        config.num_threads = args[i].parse()?;
                    }
                }
                "-u" => {
                    if i < args.len() {
                        config.user_privileges = true;
                        user::UserPrivileges::check_root()?;
                        if config.user.is_some() {
                            return Err("Error: only one -u is allowed".into());
                        }
                        i += 1;
                        config.user = Some(args[i].clone());
                    }
                    i += 1;
                }
                "-d" => {
                    config.daemon = true;
                }
                "-D" => {
                    config.debug = true;
                }
                "-w" => {
                    config.websocket = true;
                    println!("starting as websocket server");
                }
                "-v" => {
                    i += 1;
                    if i < args.len() {
                        if config.version.is_some() {
                            return Err("Error: only one -v is allowed".into());
                        }
                        if args[i] != "0.3" {
                            return Err(format!(
                                "Error: unsupported version for backwards compatibility: >{}<",
                                args[i]
                            )
                            .into());
                        }
                        config.version = Some(3);
                    }
                }
                "--help" | "-h" => {
                    print_help();
                    return Err("Help printed".into());
                }
                _ => {
                    return Err(format!("Unknown option: {}", args[i]).into());
                }
            }
            i += 1;
        }

        logger::init_logger(if config.debug {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })?;

        // Validate thread count
        if config.num_threads == 0 {
            return Err("Number of threads (-t) must be positive!".into());
        }

        // Validate TLS configuration
        // if !config.ssl_listen_addresses.is_empty() {
            if config.cert_path.is_some() && config.key_path.is_some() {
               config.ssl_listen_addresses.push( parse_listen_address("8080").unwrap());
            }
            config.listen_addresses.push( parse_listen_address("5005").unwrap());
        // }

        // Validate required options for non-TLS connections
        // if config.ssl_listen_addresses.is_empty()
        //     && (config.cert_path.is_none() || config.key_path.is_none())
        // {
        //     return Err("Error: -c and -k options are required".into());
        // }

        // if config.listen_addresses.is_empty() && config.ssl_listen_addresses.is_empty() {
        //     return Err("Error: at least one -l or -L option is required".into());
        // }

        if config.daemon {
            // Run as daemon
            daemon::daemonize()?;
        }

        Ok(config)
    }

    pub fn load_identity(&self) -> Result<TlsAcceptor, Box<dyn Error + Send + Sync>> {
        debug!("Starting TLS configuration loading");
        let certs = self.load_certs()?;
        debug!("Successfully loaded {} certificates", certs.len());
        let key = self.load_private_key()?;
        debug!("Successfully loaded private key");

        debug!("Building TLS configuration");
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("bad certificates/private key");

            debug!("TLS configuration built successfully");
        Ok(TlsAcceptor::from(Arc::new(config)))
    }

    fn load_certs(&self) -> Result<Vec<CertificateDer<'static>>, Box<dyn Error + Send + Sync>> {
        let cert_path = self.cert_path.as_ref().unwrap();
        debug!("Loading certificates from {}", cert_path);
        let certfile = fs::read(cert_path)?;
        debug!("Read {} bytes from certificate file", certfile.len());
        let mut reader = BufReader::new(certfile.as_slice());
        let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
        debug!("Successfully parsed {} certificates", certs.len());
        Ok(certs)
    }

    fn load_private_key(&self) -> Result<PrivateKeyDer<'static>, Box<dyn Error + Send + Sync>> {
        let key_path = self.key_path.as_ref().unwrap();
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
            Err("No private keys found in key file".into())
        }
    }
}

impl Default for RmbtServerConfig {
    fn default() -> Self {
        Self {
            listen_addresses: vec!["127.0.0.1:8080".parse().unwrap()],
            ssl_listen_addresses: vec![],
            cert_path: None,
            key_path: None,
            num_threads: 4,
            user: None,
            daemon: false,
            debug: false,
            websocket: false,
            version: Some(1),
            secret_keys: Vec::new(),
            secret_key_labels: Vec::new(),
            user_privileges: false,
        }
    }
}

pub fn parse_listen_address(addr: &str) -> Result<SocketAddr, Box<dyn Error + Send + Sync>> {
    println!("parse_listen_address: {}", addr);
    // Try IPv6 format: [::1]:8080
    if addr.starts_with('[') {
        if let Some(end_bracket) = addr.rfind(']') {
            let ip_str = &addr[1..end_bracket];
            if let Some(port_str) = addr[end_bracket + 1..].strip_prefix(':') {
                let ip: Ipv6Addr = ip_str.parse()?;
                let port: u16 = port_str.parse()?;
                return Ok(SocketAddr::new(IpAddr::V6(ip), port));
            }
        }
        return Err(format!("Invalid IPv6 address format: {}", addr).into());
    }

    // Try IPv4 format: 127.0.0.1:8080
    if let Some((ip, port)) = addr.split_once(':') {
        let ip: std::net::Ipv4Addr = ip.parse()?;
        let port: u16 = port.parse()?;
        return Ok(SocketAddr::new(IpAddr::V4(ip), port));
    }

    // Try port only: 8080
    if let Ok(port) = addr.parse::<u16>() {
        return Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port));
    }

    Err(format!("Invalid listen address format: {}", addr).into())
}

fn print_help() {
    println!("==== rmbtd ====");
    println!("By default, rmbtd will listen TCP on port 5005 and TLS on port 8080.");
    println!("Usage: 'nettest -s' will listen on TCP on port 5005");
    println!("Usage: 'nettest -s -k privkey1.pem -c fullchain1.pem'  will listen on TCP and TLS on ports 5005 and 8080");
    println!("Usage: nettest -s [-l <listen_address>] [-c <cert_path>] [-k <key_path>] [-t <num_threads>] [-u <user>] [-d] [-D] [-w] [-v <version>]");
    println!("command line arguments:\n");
    println!(" -l/-L  listen on (IP and) port; -L for SSL;");
    println!("        examples: \"443\",\"1.2.3.4:1234\",\"[2001:1234::567A]:1234\"");
    println!("        maybe specified multiple times; at least once\n");
    println!(" -c     path to SSL certificate in PEM format;");
    println!("        intermediate certificates following server cert in same file if needed");
    println!("        required\n");
    println!(" -k     path to SSL key file in PEM format; required\n");
    println!(" -t     number of worker threads to run for handling connections (default: 200)\n");
    println!(" -u     drop root privileges and setuid to specified user; must be root\n");
    println!(" -d     fork into background as daemon (no argument)\n");
    println!(" -D     enable debug logging (no argument)\n");
    println!(" -w     use as websocket server (no argument)\n");
    println!(" -v     behave as version (v) for serving very old clients");
    println!("        example: \"0.3\"\n");
    println!("Required are -c,-k and at least one -l/-L option");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn create_test_args(args: &[&str]) -> Vec<String> {
        let mut vec = vec!["test_program".to_string()];
        vec.extend(args.iter().map(|&s| s.to_string()));
        vec
    }

    #[test]
    fn test_parse_listen_address_ipv6() {
        let addr = parse_listen_address("[::1]:8080").unwrap();
        assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_parse_listen_address_ipv4() {
        let addr = parse_listen_address("127.0.0.1:8080").unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_parse_listen_address_port_only() {
        let addr = parse_listen_address("8080").unwrap();
        assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::UNSPECIFIED));
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_parse_listen_address_invalid() {
        assert!(parse_listen_address("invalid").is_err());
        assert!(parse_listen_address("127.0.0.1:invalid").is_err());
        assert!(parse_listen_address("[::1]:invalid").is_err());
    }

    #[test]
    fn test_validate_required_cert_and_key() {
        let args = create_test_args(&["-l", "8080"]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("-c and -k options are required"));
    }

    #[test]
    fn test_validate_positive_threads() {
        let args = create_test_args(&["-l", "8080", "-c", "cert.pem", "-k", "key.pem", "-t", "0"]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Number of threads (-t) must be positive"));
    }

    #[test]
    fn test_validate_at_least_one_port() {
        let args = create_test_args(&["-c", "cert.pem", "-k", "key.pem"]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one -l or -L option is required"));
    }

    #[test]
    fn test_validate_tls_config_without_cert() {
        let args = create_test_args(&["-L", "8443", "-k", "key.pem"]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Need path to certificate (-c) for TLS connections"));
    }

    #[test]
    fn test_validate_tls_config_without_key() {
        let args = create_test_args(&["-L", "8443", "-c", "cert.pem"]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Need path to key (-k) for TLS connections"));
    }

    #[test]
    fn test_valid_config() {
        let args = create_test_args(&[
            "-l", "8080", "-L", "8443", "-c", "cert.pem", "-k", "key.pem", "-t", "100",
        ]);
        let config = RmbtServerConfig::from_args_vec(args).unwrap();

        assert_eq!(config.listen_addresses.len(), 1);
        assert_eq!(config.ssl_listen_addresses.len(), 1);
        assert_eq!(config.listen_addresses[0].port(), 8080);
        assert_eq!(config.ssl_listen_addresses[0].port(), 8443);
        assert_eq!(config.cert_path.unwrap(), "cert.pem");
        assert_eq!(config.key_path.unwrap(), "key.pem");
        assert_eq!(config.num_threads, 100);
    }

    #[test]
    fn test_duplicate_cert() {
        let args = create_test_args(&[
            "-l",
            "8080",
            "-c",
            "cert1.pem",
            "-c",
            "cert2.pem",
            "-k",
            "key.pem",
        ]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("only one -c is allowed"));
    }

    #[test]
    fn test_duplicate_key() {
        let args = create_test_args(&[
            "-l", "8080", "-c", "cert.pem", "-k", "key1.pem", "-k", "key2.pem",
        ]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("only one -k is allowed"));
    }

    #[test]
    fn test_invalid_version() {
        let args =
            create_test_args(&["-l", "8080", "-c", "cert.pem", "-k", "key.pem", "-v", "1.0"]);
        let result = RmbtServerConfig::from_args_vec(args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unsupported version"));
    }
}
