use crate::device::Device;
use anyhow::Context;
use anyhow::{anyhow, Result};
use bus::Bus;
use std::net::UdpSocket;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

pub(crate) struct UdpDevice {
    /// Sender object to send data to UdpDevice
    sender: Sender<Vec<u8>>,
    /// Receiver object to receiver data from Udpdevice
    maybe_receiver: Option<Receiver<Vec<u8>>>,
    /// Stop signal listened by deamon threas
    stop_signal: Bus<u8>,
}

impl Drop for UdpDevice {
    // stop deamon threads when dropping the object
    fn drop(&mut self) {
        self.stop_signal.broadcast(0); // signal to stop threads
    }
}

impl Device for UdpDevice {
    fn sender(&self) -> Result<Sender<Vec<u8>>> {
        Ok(self.sender.clone())
    }

    fn receiver(self: &mut UdpDevice) -> Result<Receiver<Vec<u8>>> {
        self.maybe_receiver.take().ok_or(anyhow!(""))
    }
}

impl UdpDevice {
    /// local_addr: str in "local_ip:port" format
    /// remote_ip: str in "remote_ip:port" format
    pub fn create(local_addr: &str, remote_addr: &str) -> Result<UdpDevice, anyhow::Error> {
        let socket = UdpSocket::bind(local_addr)
            .with_context(|| format!("Failed to bind udp client to local_addr {}", local_addr))?;
        socket
            .set_nonblocking(true)
            .with_context(|| "Failed to set nonblocking mode")?;
        // Note: Udp doesn't "connect", it just record the remote addr in the object
        socket.connect(remote_addr).with_context(|| {
            format!(
                "Failed to connect udp client to remote_addr {}",
                remote_addr
            )
        })?;
        let socket_tx = socket.try_clone()?;
        let socket_rx = socket.try_clone()?;

        let mut stop_signal = Bus::new(1);
        let mut stop_signal_reader_1 = stop_signal.add_rx();
        let mut stop_signal_reader_2 = stop_signal.add_rx();

        // local to remote thread (sender thread)
        let (sender, sender_rx) = channel::<Vec<u8>>();
        thread::spawn(move || loop {
            if let Ok(pkg) = sender_rx.try_recv() {
                socket_tx.send(&pkg).unwrap();
                // println!("[sender_thread] sent: {:?}", pkg);
            }
            if stop_signal_reader_1.try_recv().is_ok() {
                // println!("[sender_thread] sender thread stopped");
                break;
            }
        });

        // remote to local thread (receiver thread)
        let (receiver_tx, receiver) = channel::<Vec<u8>>();
        thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            loop {
                if let Ok(size) = socket_rx.recv(&mut buf) {
                    // println!("[listener_thread] received: {:?}", buf[..size]);
                    receiver_tx.send(buf[..size].to_vec()).unwrap();
                }
                if stop_signal_reader_2.try_recv().is_ok() {
                    // println!("[listener_thread] receiver thread stopped");
                    break;
                }
            }
        });

        Ok(UdpDevice {
            sender,
            maybe_receiver: Some(receiver),
            stop_signal,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::device::Device;
    use crate::udpdevice::UdpDevice;
    use std::{net::UdpSocket, thread};

    #[test]
    fn test_create_udp() {
        assert!(UdpDevice::create("127.0.0.1:5000", "127.0.0.1:5001").is_ok());
        assert!(UdpDevice::create("127.0.0.1:6000", "invalid").is_err());
    }

    #[test]
    fn test_send_udp() {
        let data = [1u8, 2, 3, 4];
        let data_to_send = data.clone();
        let test_receiver = UdpSocket::bind("127.0.0.1:8001").unwrap();

        thread::spawn(move || {
            let udp_device = UdpDevice::create("127.0.0.1:8000", "127.0.0.1:8001").unwrap();
            let sender = udp_device.sender().unwrap();
            assert!(sender.send(data_to_send.to_vec()).is_ok());
        });

        let mut buf = [0u8; 4];
        assert!(test_receiver.recv(&mut buf).is_ok());
        assert_eq!(buf, data);
    }

    #[test]
    fn test_receive_udp() {
        let data = [1u8, 2, 3, 4];
        let data_to_send = data.clone();

        let mut udp_device = UdpDevice::create("127.0.0.1:7000", "127.0.0.1:7001").unwrap();

        thread::spawn(move || {
            let test_sender = UdpSocket::bind("127.0.0.1:7001").unwrap();
            test_sender
                .send_to(&data_to_send, "127.0.0.1:7000")
                .unwrap();
        });

        let receiver = udp_device.receiver().unwrap();
        let buf = receiver.recv().unwrap();
        assert_eq!(buf, vec![1u8, 2, 3, 4]);
    }
}
