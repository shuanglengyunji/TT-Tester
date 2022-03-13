use crate::workers::Worker;
use anyhow::Context;
use anyhow::{anyhow, Result};
use std::net::UdpSocket;
use std::time::Duration;

pub(crate) struct UdpWorker {
    socket: UdpSocket,
}

impl Worker for UdpWorker {
    fn send(&self, buf: Vec<u8>, timeout: Option<Duration>) -> Result<usize> {
        self.socket.set_write_timeout(timeout)?;
        self.socket
            .send(&buf)
            .or(Err(anyhow!("Failed to send data")))
    }

    fn receive(&self, timeout: Option<Duration>) -> Result<Vec<u8>> {
        let mut buf = [0u8; 2048]; // max 2k
        self.socket.set_read_timeout(timeout)?;
        let size = self.socket.recv(&mut buf)?;
        Ok(buf[..size].to_vec())
    }

    fn create(port: &str) -> Result<Box<(dyn Worker)>, anyhow::Error> {
        // port must in this format: udp,local_ip:port,remote_ip:port

        let list: Vec<&str> = port.split(',').collect();
        assert_eq!(list.len(), 3, "port invalid: {}", port);

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

#[cfg(test)]
mod test {
    use crate::{udpworker::UdpWorker, workers::Worker};
    use serial_test::serial;
    use std::{net::UdpSocket, thread, time::Duration};

    #[test]
    #[serial]
    fn test_send_udp() {
        let test_receiver = UdpSocket::bind("127.0.0.1:8001").unwrap();

        thread::spawn(|| {
            let buf = [1u8, 2, 3, 4];
            let udpworker = UdpWorker::create("udp,127.0.0.1:8000,127.0.0.1:8001").unwrap();
            assert!(udpworker
                .send(buf.to_vec(), Some(Duration::from_millis(100)))
                .is_ok());
        });

        let mut buf = [0u8; 4];
        test_receiver.recv(&mut buf).unwrap();
        assert_eq!(buf, [1u8, 2, 3, 4]);
    }

    #[test]
    #[serial]
    fn test_send_udp_to_invalid_url() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let udpworker: Box<dyn Worker> = Box::new(UdpWorker { socket });
        assert!(udpworker
            .send(vec![1u8, 2, 3, 4], Some(Duration::from_millis(100)))
            .is_err());
    }

    #[test]
    #[serial]
    fn test_receive_udp() {
        let udpworker = UdpWorker::create("udp,127.0.0.1:8000,127.0.0.1:8001").unwrap();

        thread::spawn(|| {
            let test_sender = UdpSocket::bind("127.0.0.1:8001").unwrap();
            let buf = [1u8, 2, 3, 4];
            test_sender.send_to(&buf, "127.0.0.1:8000").unwrap();
        });

        let buf = udpworker.receive(Some(Duration::from_millis(100))).unwrap();
        assert_eq!(buf, vec![1u8, 2, 3, 4]);
    }

    #[test]
    #[serial]
    fn test_receive_udp_timeout() {
        let udpworker = UdpWorker::create("udp,127.0.0.1:8000,127.0.0.1:8001").unwrap();
        assert!(udpworker.receive(Some(Duration::from_millis(100))).is_err());
    }
}
