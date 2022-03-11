use anyhow::Context;
use anyhow::{anyhow, Result};
use std::net::UdpSocket;

pub trait Worker {
    fn send(&self);
}

struct UdpWorker {
    socket: UdpSocket,
}

impl Worker for UdpWorker {
    fn send(&self) {
        let buf = [0u8, 1, 2, 3];
        self.socket.send(&buf).expect(
            format!(
                "Udp worker cannot send package from local address {:?} to dest address {:?}",
                self.socket.local_addr(),
                self.socket.peer_addr()
            )
            .as_str(),
        );
    }
}

impl UdpWorker {
    fn create(port: &str) -> Result<Box<(dyn Worker)>, anyhow::Error> {
        // port must in this format: udp,local_ip:port,remote_ip:port

        let list: Vec<&str> = port.split(',').collect();
        if list.len() != 3 {
            return Err(anyhow!("port invalid"));
        }

        let socket = UdpSocket::bind(list[1]).with_context(|| "Failed to bind udp client")?;

        // Unlike in the TCP case, passing an array of addresses to the connect function
        // of a UDP socket is not a useful thing to do: The OS will be unable to determine
        // whether something is listening on the remote address without the application
        // sending data.
        socket
            .connect(list[2])
            .with_context(|| "Failed to connect remote port")?;

        Ok(Box::new(UdpWorker { socket }))
    }
}

pub fn create_worker(port: &str) -> Result<Box<dyn Worker>> {
    if port.starts_with("udp") {
        UdpWorker::create(port)
    } else {
        Err(anyhow!(""))
    }
}

#[cfg(test)]
mod test {
    use super::create_worker;

    #[test]
    fn test_create_udp_worker() {
        assert!(create_worker("udp,127.0.0.1:8000,127.0.0.1:8001").is_ok());
    }

    #[test]
    fn test_create_udp_worker_without_local_port() {
        assert!(create_worker("udp,,127.0.0.1:8001").is_err());
    }

    #[test]
    fn test_create_udp_worker_from_invalid_url() {
        assert!(create_worker("invalid,127.0.0.1:8000,127.0.0.1:8001").is_err());
    }
}
