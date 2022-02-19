use anyhow::Context;
use anyhow::{anyhow, Result};
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use url::{Position, Url};

pub trait Worker {
    fn create(port: Vec<Url>) -> Result<Box<dyn Worker>>
    where
        Self: Sized;

    fn send(&self);
}

struct UdpWorker {
    dest: SocketAddr,
    local: SocketAddr,
    socket: UdpSocket,
}

impl Worker for UdpWorker {
    fn create(port: Vec<Url>) -> Result<Box<(dyn Worker + 'static)>, anyhow::Error> {
        let (socket, local) = if let Some(url) = port.get(1) {
            let local = *&url[Position::BeforeHost..Position::AfterPath]
                .to_socket_addrs()?
                .next()
                .ok_or(anyhow!("Local address invalid in udp worker"))?;
            let socket = UdpSocket::bind(local).with_context(|| {
                format!("Couldn't create udp worker with local address {}", local)
            })?;
            (socket, local)
        } else {
            let socket = UdpSocket::bind("0.0.0.0:0")
                .with_context(|| "Couldn't create udp worker with random local port")?;
            let local = socket.local_addr()?;
            (socket, local)
        };

        let dest = *&port[0][Position::BeforeHost..Position::AfterPath]
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Dest address invalid in udp worker"))?;

        // Unlike in the TCP case, passing an array of addresses to the connect function
        // of a UDP socket is not a useful thing to do: The OS will be unable to determine
        // whether something is listening on the remote address without the application
        // sending data.
        socket.connect(dest).with_context(|| {
            format!(
                "Couldn't connect udp worker with local address {} to dest address {}",
                local, dest
            )
        })?;

        Ok(Box::new(UdpWorker {
            dest,
            local,
            socket,
        }))
    }

    fn send(&self) {
        let buf = [0u8, 1, 2, 3];
        self.socket.send(&buf).expect(
            format!(
                "Udp worker cannot send package from local address {} to dest address {}",
                self.local, self.dest
            )
            .as_str(),
        );
    }
}

pub fn create_worker(port: Vec<Url>) -> Result<Box<dyn Worker>> {
    match port[0].scheme() {
        // "serial" => todo!(),
        "udp" => UdpWorker::create(port),
        // "tcpclient" => todo!(),
        _ => Err(anyhow!(""))?,
    }
}

#[cfg(test)]
mod test {
    use url::Url;

    use super::create_worker;

    #[test]
    fn test_create_worker() {
        let dest_port = Url::parse("udp://127.0.0.1:8001").unwrap();
        let local_port = Url::parse("udp://127.0.0.1:0").unwrap();
        let port = vec![dest_port, local_port];
        assert!(create_worker(port).is_ok());
    }

    #[test]
    fn test_create_worker_from_invalid_url() {
        let dest_port = Url::parse("pdu://127.0.0.1:8001").unwrap();
        let local_port = Url::parse("pdu://127.0.0.1:0").unwrap();
        let port = vec![dest_port, local_port];
        assert!(create_worker(port).is_err());
    }
}
