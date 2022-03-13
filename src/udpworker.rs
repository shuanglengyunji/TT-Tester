use crate::workers::Worker;
use anyhow::Context;
use anyhow::{anyhow, Result};
use bus::Bus;
use std::net::UdpSocket;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

pub(crate) struct UdpWorker {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    bus: Bus<u8>,
}

impl Drop for UdpWorker {
    fn drop(&mut self) {
        self.bus.broadcast(0);
    }
}

impl Worker for UdpWorker {
    fn send(&self, buf: Vec<u8>, _timeout: Option<Duration>) -> Result<()> {
        self.tx.send(buf).or(Err(anyhow!("Failed to send data")))
    }

    fn receive(&self, timeout: Option<Duration>) -> Result<Vec<u8>> {
        if let Some(timeout) = timeout {
            self.rx
                .recv_timeout(timeout)
                .or(Err(anyhow!("Failed to receive data")))
        } else {
            self.rx.recv().or(Err(anyhow!("Failed to receive data")))
        }
    }

    fn create(port: &str) -> Result<Box<(dyn Worker)>, anyhow::Error> {
        // port must in this format: udp,local_ip:port,remote_ip:port

        let list: Vec<&str> = port.split(',').collect();
        assert_eq!(list.len(), 3, "port invalid: {}", port);

        let socket = UdpSocket::bind(list[1]).with_context(|| "Failed to bind udp client")?;
        socket
            .set_nonblocking(true)
            .with_context(|| "Failed to set nonblocking mode")?;

        // Unlike in the TCP case, passing an array of addresses to the connect function
        // of a UDP socket is not a useful thing to do: The OS will be unable to determine
        // whether something is listening on the remote address without the application
        // sending data.
        socket
            .connect(list[2])
            .with_context(|| "Failed to connect remote port")?;

        let mut bus = Bus::new(1);

        let mut bus_tx = bus.add_rx();
        let socket_tx = socket.try_clone()?;
        let (sender_tx, sender_rx) = channel::<Vec<u8>>();
        thread::spawn(move || loop {
            if let Ok(pkg) = sender_rx.try_recv() {
                socket_tx.send(&pkg).unwrap();
                println!("[sender_thread] sent: {:?}", pkg);
            }
            if bus_tx.try_recv().is_ok() {
                println!("[sender_thread] sender thread stopped");
                break;
            }
        });

        let mut bus_rx = bus.add_rx();
        let socket_rx = socket.try_clone()?;
        let (listener_tx, listener_rx) = channel::<Vec<u8>>();
        thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            loop {
                if let Ok(size) = socket_rx.recv(&mut buf) {
                    let received = buf[..size].to_vec();
                    println!("[listener_thread] received: {:?}", received);
                    listener_tx.send(received).unwrap();
                }
                if bus_rx.try_recv().is_ok() {
                    println!("[listener_thread] listener thread stopped");
                    break;
                }
            }
        });

        Ok(Box::new(UdpWorker {
            tx: sender_tx,
            rx: listener_rx,
            bus,
        }))
    }
}

#[cfg(test)]
mod test {
    use crate::{udpworker::UdpWorker, workers::Worker};
    use std::{net::UdpSocket, thread, time::Duration};

    #[test]
    fn test_create_udp_with_invalid_remote_port() {
        assert!(UdpWorker::create("udp,127.0.0.1:8000,invalid:8001").is_err())
    }

    #[test]
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
    fn test_receive_udp_timeout() {
        let udpworker = UdpWorker::create("udp,127.0.0.1:0,127.0.0.1:5000").unwrap();
        assert!(udpworker.receive(Some(Duration::from_millis(100))).is_err());
    }
}
