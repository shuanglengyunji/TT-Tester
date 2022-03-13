use crate::udpworker::UdpWorker;
use crate::serialworker::SerialWorker;
use anyhow::{anyhow, Result};

use std::time::Duration;

pub trait Worker {
    fn send(&self, buf: Vec<u8>, timeout: Option<Duration>) -> Result<usize>;
    fn receive(&self, timeout: Option<Duration>) -> Result<Vec<u8>>;
    fn create(port: &str) -> Result<Box<(dyn Worker)>, anyhow::Error>
    where
        Self: Sized;
}

pub fn create_worker(port: &str) -> Result<Box<dyn Worker>> {
    if port.starts_with("udp") {
        UdpWorker::create(port)
    } else if port.starts_with("serial") {
        SerialWorker::create(port)
    } else {
        Err(anyhow!("No compatible worker found"))
    }
}

#[cfg(test)]
mod test {
    use super::create_worker;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_create_udp_worker() {
        assert!(create_worker("udp,127.0.0.1:8000,127.0.0.1:8001").is_ok());
    }

    #[test]
    #[serial]
    fn test_create_udp_worker_without_local_port() {
        assert!(create_worker("udp,,127.0.0.1:8001").is_err());
    }

    #[test]
    #[serial]
    fn test_create_udp_worker_from_invalid_url() {
        assert!(create_worker("invalid,127.0.0.1:8000,127.0.0.1:8001").is_err());
    }
}
