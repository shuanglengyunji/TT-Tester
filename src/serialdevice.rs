use crate::device::Device;
use anyhow::Context;
use anyhow::{anyhow, Result};
use bus::Bus;
use serialport;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

pub(crate) struct SerialDevice {
    /// Sender object to send data to UdpDevice
    sender: Sender<Vec<u8>>,
    /// Receiver object to receiver data from Udpdevice
    maybe_receiver: Option<Receiver<Vec<u8>>>,
    /// Stop signal listened by deamon threas
    stop_signal: Bus<u8>,
}

impl Drop for SerialDevice {
    fn drop(&mut self) {
        self.stop_signal.broadcast(0);
    }
}

impl Device for SerialDevice {
    fn sender(&self) -> Result<Sender<Vec<u8>>> {
        Ok(self.sender.clone())
    }

    fn receiver(self: &mut SerialDevice) -> Result<Receiver<Vec<u8>>> {
        self.maybe_receiver.take().ok_or(anyhow!(""))
    }
}

impl SerialDevice {
    fn create(device: &str, baud_rate: u32) -> Result<SerialDevice> {
        let serialport = serialport::new(device, baud_rate)
            .open()
            .with_context(|| "Failed to open serialport")?;
        let mut serialport_tx = serialport.try_clone()?;
        let mut serialport_rx = serialport.try_clone()?;

        let mut stop_signal = Bus::new(1);
        let mut stop_signal_reader_1 = stop_signal.add_rx();
        let mut stop_signal_reader_2 = stop_signal.add_rx();

        // sender thread
        let (sender, sender_rx) = channel::<Vec<u8>>();
        thread::spawn(move || loop {
            if let Ok(pkg) = sender_rx.try_recv() {
                serialport_tx.write(&pkg).unwrap();
            }
            if stop_signal_reader_1.try_recv().is_ok() {
                break;
            }
        });

        // receiver thread
        let (receiver_tx, receiver) = channel::<Vec<u8>>();
        thread::spawn(move || loop {
            let mut buf = [0u8; 2048]; // max 2k
            if let Ok(size) = serialport_rx.read(&mut buf) {
                receiver_tx.send(buf[..size].to_vec()).unwrap();
            }
            if stop_signal_reader_2.try_recv().is_ok() {
                break;
            }
        });

        Ok(SerialDevice {
            sender,
            maybe_receiver: Some(receiver),
            stop_signal,
        })
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod test {
    use crate::{device::Device, serialdevice::SerialDevice};
    use serialport;
    use std::thread;

    #[test]
    fn test_create_serial() {
        assert!(SerialDevice::create("/tmp/serial1", 115200).is_ok());
        assert!(SerialDevice::create("/dev/invalid", 115200).is_err());
    }

    #[test]
    fn test_send_serial() {
        let data = [1u8, 2, 3, 4];
        let mut test_receiver = serialport::new("/tmp/serial2", 115200).open().unwrap();

        thread::spawn(move || {
            let serial_device = SerialDevice::create("/tmp/serial1", 115200).unwrap();
            let sender = serial_device.sender().unwrap();
            assert!(sender.send(data.to_vec()).is_ok());
        });

        let mut buf = [0u8; 4]; // max 2k
        loop {
            if let Ok(size) = test_receiver.read(&mut buf) {
                assert_eq!(size, data.len());
                assert_eq!(data, buf);
                break;
            }
        }
    }

    #[test]
    fn test_receive_serial() {
        let data = [1u8, 2, 3, 4];
        let mut serial_device = SerialDevice::create("/tmp/serial1", 115200).unwrap();

        thread::spawn(move || {
            let mut test_sender = serialport::new("/tmp/serial2", 115200).open().unwrap();
            test_sender.write_all(&data).unwrap();
        });

        let receiver = serial_device.receiver().unwrap();
        let buf = receiver.recv().unwrap();
        assert_eq!(data.to_vec(), buf);
    }
}
