use crate::workers::Worker;
use anyhow::Context;
use anyhow::{anyhow, Result};
use bus::Bus;
use serialport;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

pub(crate) struct SerialWorker {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    bus: Bus<u8>,
}

impl Drop for SerialWorker {
    fn drop(&mut self) {
        self.bus.broadcast(0);
    }
}

impl Worker for SerialWorker {
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
        // port must in this format: serial,path_to_serial_device:baudrate

        let list: Vec<&str> = port.split(&[',', ':'][..]).collect();
        assert_eq!(list.len(), 3, "port invalid: {}", port);

        let serialport = serialport::new(list[1], list[2].parse()?)
            .open()
            .with_context(|| "Failed to open serialport")?;

        let mut bus = Bus::new(1);

        let mut bus_tx = bus.add_rx();
        let mut serialport_tx = serialport.try_clone()?;
        let (sender_tx, sender_rx) = channel::<Vec<u8>>();
        thread::spawn(move || loop {
            if let Ok(pkg) = sender_rx.try_recv() {
                serialport_tx.write(&pkg).unwrap();
            }
            if bus_tx.try_recv().is_ok() {
                break;
            }
        });

        let mut bus_rx = bus.add_rx();
        let mut serialport_rx = serialport.try_clone()?;
        let (listener_tx, listener_rx) = channel::<Vec<u8>>();
        thread::spawn(move || loop {
            if let Ok(bytes) = serialport_rx.bytes_to_read() {
                if bytes > 0 {
                    let mut buf = Vec::new();
                    serialport_rx.read_to_end(&mut buf).unwrap();
                    listener_tx.send(buf).unwrap();
                }
            }
            if bus_rx.try_recv().is_ok() {
                break;
            }
        });

        Ok(Box::new(SerialWorker {
            tx: sender_tx,
            rx: listener_rx,
            bus,
        }))
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod test {
    use crate::{serialworker::SerialWorker, workers::Worker};
    use std::process::Command;
    use std::{process::Child, thread, time::Duration};

    use serialport;
    use test_context::{test_context, TestContext};

    struct VirtualSerialPort {
        child: Child,
    }

    impl TestContext for VirtualSerialPort {
        fn setup() -> VirtualSerialPort {
            let child = Command::new("socat")
                .arg("-dd")
                .arg("pty,raw,echo=0,link=/tmp/serial1")
                .arg("pty,raw,echo=0,link=/tmp/serial2")
                .spawn()
                .expect("Failed to setup virtual serial port process");
            thread::sleep(Duration::from_millis(500)); // wait for socat to setup virtual port
            VirtualSerialPort { child }
        }

        fn teardown(mut self) {
            self.child
                .kill()
                .expect("Failed to stop virtual serial port process");
        }
    }

    #[test]
    #[test_context(VirtualSerialPort)]
    fn test_create_serial() {
        assert!(SerialWorker::create("serial,/tmp/serial1,115200").is_ok());
    }

    #[test]
    #[test_context(VirtualSerialPort)]
    fn test_create_serial_with_invalid_device() {
        assert!(SerialWorker::create("serial,/dev/invalid,115200").is_err())
    }

    #[test]
    #[test_context(VirtualSerialPort)]
    fn test_create_serial_with_invalid_baudrate() {
        assert!(SerialWorker::create("serial,/tmp/serial1,invalid").is_err())
    }

    #[test]
    #[test_context(VirtualSerialPort)]
    fn test_send_serial() {
        let buf = [1u8, 2, 3, 4];
        let serialworker = SerialWorker::create("serial,/tmp/serial1,115200").unwrap();
        assert!(serialworker.send(buf.to_vec(), None).is_ok());
    }

    #[test]
    #[test_context(VirtualSerialPort)]
    #[ignore = "Bug in serialport"]
    fn test_receive_serial() {
        let serialworker = SerialWorker::create("serial,/tmp/serial1,115200").unwrap();

        thread::spawn(|| {
            let mut buf = [1u8, 2, 3, 4];
            let mut test_sender = serialport::new("/tmp/serial2", 115200).open().unwrap();
            test_sender.write_all(&mut buf).unwrap();
        });

        let buf = serialworker.receive(Some(Duration::from_millis(100))).unwrap();
        assert_eq!(buf, vec![1u8, 2, 3, 4]);
    }

    #[test]
    #[test_context(VirtualSerialPort)]
    fn test_receive_serial_timeout() {
        let serialworker = SerialWorker::create("serial,/tmp/serial1,115200").unwrap();
        assert!(serialworker.receive(Some(Duration::from_millis(100))).is_err());
    }
}
