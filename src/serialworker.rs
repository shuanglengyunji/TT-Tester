use std::time::Duration;

use serialport::SerialPort;
use anyhow::Context;
use anyhow::{anyhow, Result};
use crate::workers::Worker;

pub(crate) struct SerialWorker {
    serial: Box<dyn SerialPort>,
}

impl Worker for SerialWorker {
    fn send(self: &SerialWorker, buf: Vec<u8>, timeout: Option<Duration>) -> Result<usize> {
        todo!()
    }

    fn receive(&self, timeout: Option<Duration>) -> Result<Vec<u8>> {
        todo!()
    }

    fn create(port: &str) -> Result<Box<(dyn Worker)>, anyhow::Error> {
        // port must in this format: serial,path_to_serial_device:baudrate

        let list: Vec<&str> = port.split(&[',', ':'][..]).collect();
        assert_eq!(list.len(), 3, "port invalid: {}", port);

        let serial = serialport::new(list[1], list[2].parse()?)
            .open()
            .with_context(|| "Failed to open serialport")?;

        Ok(Box::new(SerialWorker { serial }))
    }
}