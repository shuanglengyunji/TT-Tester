use anyhow::Context;
use anyhow::{anyhow, Result};
use std::net::UdpSocket;
use url::{Position, Url};

pub trait Worker {
    fn create(remote_port: Url, local_port: Option<Url>) -> Result<Box<dyn Worker>>
    where
        Self: Sized;

    fn send(&self);
}

struct UdpWorker {
    socket: UdpSocket,
}

impl Worker for UdpWorker {
    fn create(
        remote_port: Url,
        local_port: Option<Url>,
    ) -> Result<Box<(dyn Worker + 'static)>, anyhow::Error> {
        let socket = UdpSocket::bind(
            &local_port.clone().unwrap_or(Url::parse("udp://0.0.0.0:0")?)
                [Position::BeforeHost..Position::AfterPath],
        )?;

        // Unlike in the TCP case, passing an array of addresses to the connect function
        // of a UDP socket is not a useful thing to do: The OS will be unable to determine
        // whether something is listening on the remote address without the application
        // sending data.
        socket
            .connect(&remote_port[Position::BeforeHost..Position::AfterPath])
            .with_context(|| {
                format!(
                    "Couldn't connect udp worker with local address {:?} to dest address {}",
                    local_port, remote_port
                )
            })?;

        Ok(Box::new(UdpWorker { socket }))
    }

    fn send(&self) {
        let buf = [0u8, 1, 2, 3];
        self.socket.send(&buf).expect(
            format!(
                "Udp worker cannot send package from local address {:?} to dest address {:?}",
                self.socket.local_addr(), self.socket.peer_addr()
            )
            .as_str(),
        );
    }
}

pub fn create_worker(remote_port: Url, local_port: Option<Url>) -> Result<Box<dyn Worker>> {
    match remote_port.scheme() {
        // "serial" => todo!(),
        "udp" => UdpWorker::create(remote_port, local_port),
        // "tcpclient" => todo!(),
        _ => Err(anyhow!(""))?,
    }
}

#[cfg(test)]
mod test {
    use url::Url;

    use super::create_worker;

    #[test]
    fn test_create_udp_worker() {
        let remote_port = Url::parse("udp://127.0.0.1:8001").unwrap();
        let local_port = Some(Url::parse("udp://127.0.0.1:0").unwrap());
        assert!(create_worker(remote_port, local_port).is_ok());
    }

    #[test]
    fn test_create_udp_worker_without_local_port() {
        let remote_port = Url::parse("udp://127.0.0.1:8001").unwrap();
        let local_port = None;
        assert!(create_worker(remote_port, local_port).is_ok());
    }

    #[test]
    fn test_create_udp_worker_from_invalid_url() {
        let remote_port = Url::parse("invalid://127.0.0.1:8001").unwrap();
        let local_port = Some(Url::parse("invalid://127.0.0.1:0").unwrap());
        assert!(create_worker(remote_port, local_port).is_err());
    }
}
