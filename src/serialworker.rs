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
