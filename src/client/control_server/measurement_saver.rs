use crate::client::client::{ClientConfig};
use log::{warn, info};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use std::fs;
use std::path::PathBuf;
use std::env;

#[derive(Debug, Clone, Copy)]
pub enum ConnectionType {
    TCP,
    TLS,
    WS,
    WSS,
}

impl ConnectionType {
    fn as_str(&self) -> &'static str {
        match self {
            ConnectionType::TCP => "TCP",
            ConnectionType::TLS => "TLS",
            ConnectionType::WS => "WS",
            ConnectionType::WSS => "WSS",
        }
    }
}

pub struct MeasurementSaver {
    client_uuid: Option<String>,
    connection_type: ConnectionType,
    threads_number: u32,
    git_hash: Option<String>,
    client_config: ClientConfig,  // Add client configuration
}

impl MeasurementSaver {
    pub fn new(client_config: &ClientConfig) -> Self {
        // Determine connection type based on configuration
        let connection_type = if client_config.use_websocket {
            if client_config.use_tls {
                ConnectionType::WSS
            } else {
                ConnectionType::WS
            }
        } else if client_config.use_tls {
            ConnectionType::TLS
        } else {
            ConnectionType::TCP
        };

        Self {
            client_uuid: client_config.client_uuid.clone(),
            connection_type,
            threads_number: client_config.thread_count as u32,
            git_hash: client_config.git_hash.clone(),
            client_config: client_config.clone(),
        }
    }

    fn resolve_server_ip(&self) -> Option<String> {
        if let Some(server_addr) = &self.client_config.server {
            // Resolve IP if it's a hostname
            if crate::client::control_server::servers::is_ip_address(server_addr) {
                Some(server_addr.clone())
            } else {
                match crate::client::control_server::servers::resolve_ip_from_web_address(server_addr) {
                    Ok(ip) => Some(ip),
                    Err(_) => Some(server_addr.clone()), // Fallback to original if resolution fails
                }
            }
        } else {
            None // Return null instead of fallback IP
        }
    }

    fn ensure_client_uuid(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        // If client_uuid already exists, return it
        if let Some(uuid) = &self.client_uuid {
            return Ok(uuid.clone());
        }
        
        // Generate new UUID
        let new_uuid = Uuid::new_v4().to_string();
        
        // Determine config file path
        let config_path = if cfg!(target_os = "macos") {
            let home_dir = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(format!("{}/.config/nettest.conf", home_dir))
        } else {
            PathBuf::from("/etc/nettest.conf")
        };
        
        // Read current file content
        let mut content = if config_path.exists() {
            fs::read_to_string(&config_path)?
        } else {
            String::new()
        };
        
        // Check if client_uuid already exists (regardless of whether it's commented out)
        let has_uuid = content.lines().any(|line| {
            let line = line.trim();
            line.starts_with("client_uuid") && !line.starts_with("#")
        });
        
        // If client_uuid doesn't exist or is commented out, add it
        if !has_uuid {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&format!("client_uuid=\"{}\"\n", new_uuid));
            
            // Write updated content
            fs::write(&config_path, content)?;
            println!("Generated and saved new client UUID: {}", new_uuid);
        }
        
        // Update internal state
        self.client_uuid = Some(new_uuid.clone());
        
        Ok(new_uuid)
    }

    pub async fn save_measurement_with_speeds(
        &mut self,
        ping_median: Option<u64>,
        download_speed_gbps: Option<f64>,
        upload_speed_gbps: Option<f64>,
        signed_data: Vec<Option<String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure client_uuid exists
        let client_uuid = self.ensure_client_uuid()?;

        // Convert speeds from Gbps to hundredths of Mbps (e.g.: 57.3 Gbps -> 5730)
        let download_speed = download_speed_gbps.map(|speed| (speed * 100.0) as i32);
        let upload_speed = upload_speed_gbps.map(|speed| (speed * 100.0) as i32);

        // Save ping in nanoseconds for greater precision
        let ping_median_ns = ping_median;

        // Generate openTestUuid - use GITHUB_SHA if available, otherwise generate
        let open_test_uuid = Uuid::new_v4().to_string();

        // Get current time
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Form data for sending
        let mut measurement_data = json!({
            "openTestUuid": open_test_uuid,
            "clientUuid": client_uuid,
            "speedDownload": download_speed,
            "speedUpload": upload_speed,
            "pingMedian": ping_median_ns,
            "time": current_time,
            "clientVersion": "2.0.0",
            "connectionType": self.connection_type.as_str(),
            "threadsNumber": self.threads_number,
            "signedData": if self.client_config.signed_result { Some(signed_data) } else { None },
            "measurementServerIp": self.resolve_server_ip(),
        });

        // Add commitHash only if git_hash exists in configuration
        if let Some(git_hash) = &self.git_hash {
            measurement_data["commitHash"] = json!(git_hash);
        } else if let Ok(commit_hash) = std::env::var("GITHUB_SHA") {
            measurement_data["commitHash"] = json!(commit_hash);
        }

        info!("Final measurement data: {:?}", measurement_data);

        info!("Saving measurement: {:?}", measurement_data);

        // Send POST request
        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/measurement/save", self.client_config.control_server))
            .header("Content-Type", "application/json")
            .header("x-nettest-client", "nt")
            .json(&measurement_data)
            .send()
            .await?;

        if response.status().is_success() {
            info!("Measurement saved successfully");
        } else {
            warn!("Failed to save measurement: HTTP {}", response.status());
            if let Ok(error_text) = response.text().await {
                warn!("Error response: {}", error_text);
            }
        }

        Ok(())
    }
}
