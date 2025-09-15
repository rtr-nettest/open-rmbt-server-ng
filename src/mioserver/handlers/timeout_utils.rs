use crate::mioserver::server::TestState;
use std::time::Duration;

// Connection timeout constant
pub const CONNECTION_TIMEOUT_SECS: u64 = 60;
// Check timeout every 1000 iterations to avoid frequent time calls
pub const TIMEOUT_CHECK_INTERVAL: u32 = 10000;

// Check connection timeout helper function
pub fn check_timeout_periodic(state: &mut TestState, function_name: &str) -> Result<usize, std::io::Error> {
    state.loop_iteration_count += 1;
    
    // Only check timeout every 1000 iterations
    if state.loop_iteration_count % TIMEOUT_CHECK_INTERVAL == 0 {
        if state.connection_start.elapsed() > Duration::from_secs(CONNECTION_TIMEOUT_SECS) {
            log::debug!("Connection timeout in {}, age: {:?}, iterations: {}", 
                       function_name, state.connection_start.elapsed(), state.loop_iteration_count);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Connection timeout"));
        }
    }
    Ok(0) // Continue processing
}
