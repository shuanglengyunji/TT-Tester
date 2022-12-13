use std::{
    io::{Read, Write},
    net::TcpStream,
    thread::{self, JoinHandle},
    time::{self, Duration},
};

use signal_hook::{consts::SIGINT, iterator::Signals};

use anyhow::{Context, Result};
use bus::Bus;
use clap::{Arg, Command};

struct TcpDevice {
    stop_signal: Bus<u8>,
    tx: Option<JoinHandle<()>>,
    rx: Option<JoinHandle<()>>,
}

impl TcpDevice {
    fn create(config: &str, bytes_per_second: usize, func: fn(usize) -> u8) -> Result<TcpDevice> {
        let tcp = TcpStream::connect(config)
            .with_context(|| format!("Failed to connect to remote_ip {}", config))?;
        tcp.set_nodelay(true)?; // no write package grouping
        tcp.set_write_timeout(None)?; // blocking write
        tcp.set_read_timeout(Some(time::Duration::from_millis(10)))?; // unblocking read

        let mut tcp_tx = tcp.try_clone()?;
        let mut tcp_rx = tcp.try_clone()?;

        let mut stop_signal: Bus<u8> = Bus::new(1);
        let mut stop_tx = stop_signal.add_rx();
        let mut stop_rx = stop_signal.add_rx();

        // tcp tx
        let tx = thread::spawn(move || {
            let mut index = 0;
            println!("tcp tx starts");
            loop {
                let start = time::SystemTime::now();

                if stop_tx.try_recv().is_ok() {
                    break;
                }

                let bytes_per_100ms = bytes_per_second / 10;
                let n = tcp_tx
                    .write(
                        &(index..(index + bytes_per_100ms))
                            .map(func)
                            .collect::<Vec<u8>>(),
                    )
                    .unwrap();
                assert_eq!(n, bytes_per_100ms);
                index = index + bytes_per_100ms;

                let time_to_100ms = Duration::from_millis(100) - start.elapsed().unwrap();
                if time_to_100ms > Duration::ZERO {
                    thread::sleep(time_to_100ms)
                } else {
                    panic!("Data rate too high");
                }
            }
            println!("tcp tx stopped");
        });

        // tcp rx
        let rx = thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            let mut index = 0;
            println!("tcp rx starts");
            loop {
                if let Ok(n) = tcp_rx.read(&mut buf) {
                    buf[..n].iter().enumerate().for_each(|(i, x)| {
                        if func(index + i) != *x {
                            panic!("Mismatch data");
                        }
                    });
                    index = index + n;
                    println!("TCP index: {:?}", index);
                }

                if stop_rx.try_recv().is_ok() {
                    break;
                }
                thread::sleep(time::Duration::from_millis(10));
            }
            println!("tcp rx stopped");
        });

        Ok(TcpDevice {
            stop_signal,
            tx: Some(tx),
            rx: Some(rx),
        })
    }

    fn stop(&mut self) {
        self.stop_signal.broadcast(0);
        self.tx.take().unwrap().join().unwrap();
        self.rx.take().unwrap().join().unwrap();
    }
}

struct SerialDevice {
    stop_signal: Bus<u8>,
    tx: Option<JoinHandle<()>>,
    rx: Option<JoinHandle<()>>,
}

impl SerialDevice {
    fn create(
        config: &str,
        bytes_per_second: usize,
        func: fn(usize) -> u8,
    ) -> Result<SerialDevice> {
        let mut serial_iter = config.split(':');
        let device = serial_iter.next().unwrap();
        let baud_rate = serial_iter.next().unwrap().parse::<u32>().unwrap();

        let mut stop_signal: Bus<u8> = Bus::new(1);
        let mut stop_tx = stop_signal.add_rx();
        let mut stop_rx = stop_signal.add_rx();

        let serialport = serialport::new(device, baud_rate).open().with_context(|| {
            format!(
                "Failed to open serialport device {} with baud rate {}",
                device, baud_rate
            )
        })?;
        let mut serialport_tx = serialport.try_clone()?;
        let mut serialport_rx = serialport.try_clone()?;

        // serial tx
        let tx = thread::spawn(move || {
            let mut index = 0;
            println!("serial tx starts");
            loop {
                let start = time::SystemTime::now();

                if stop_tx.try_recv().is_ok() {
                    break;
                }

                let bytes_per_100ms = bytes_per_second / 10;
                let n = serialport_tx
                    .write(
                        &(index..(index + bytes_per_100ms))
                            .map(func)
                            .collect::<Vec<u8>>(),
                    )
                    .unwrap();
                assert_eq!(n, bytes_per_100ms);
                index = index + bytes_per_100ms;

                let time_to_100ms = Duration::from_millis(100) - start.elapsed().unwrap();
                if time_to_100ms > Duration::ZERO {
                    thread::sleep(time_to_100ms)
                } else {
                    panic!("Data rate too high");
                }
            }
            println!("serial tx stopped");
        });

        // serial rx
        let rx = thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            let mut index = 0;
            println!("serial rx starts");
            loop {
                if let Ok(n) = serialport_rx.read(&mut buf[..]) {
                    buf[..n].iter().enumerate().for_each(|(i, x)| {
                        if func(index + i) != *x {
                            panic!("Mismatch data");
                        }
                    });
                    index = index + n;
                    println!("Serial index: {:?}", index);
                }

                if stop_rx.try_recv().is_ok() {
                    break;
                }
                thread::sleep(time::Duration::from_millis(10));
            }
            println!("serial rx stopped");
        });

        Ok(SerialDevice {
            stop_signal,
            tx: Some(tx),
            rx: Some(rx),
        })
    }

    fn stop(&mut self) {
        self.stop_signal.broadcast(0);
        self.tx.take().unwrap().join().unwrap();
        self.rx.take().unwrap().join().unwrap();
    }
}

fn main() -> Result<()> {
    let m = Command::new("ser2tcp-tester")
        .version(clap::crate_version!())
        .about("Speed tester for transparent transmission between tcp and serial port")
        .arg(
            Arg::new("serial")
                // .required(true)
                .short('s')
                .long("serial")
                .value_name("DEVICE:BAUD_RATE")
                .help("Serial port device, for example: /dev/ttyUSB0:115200 (Linux) or COM1:115200 (Windows)"),
        )
        .arg(
            Arg::new("tcp")
                .short('t')
                .required(true)
                .long("tcp")
                .value_name("ADDRESS:PORT")
                .help("Tcp port, for example: 192.168.7.1:8000"),
        )
        .get_matches();

    let mut tcp_device = TcpDevice::create(
        m.get_one::<String>("tcp").expect("tcp config is required"),
        1440,
        |_| 1,
    )?;
    let mut serial_device = SerialDevice::create(
        m.get_one::<String>("serial")
            .expect("serial config is required"), 1440, |_| 1
    )?;

    let mut signals = Signals::new(&[SIGINT])?;
    signals.wait();

    tcp_device.stop();
    serial_device.stop();

    Ok(())
}

#[cfg(test)]
mod test {
    use core::time;
    use std::thread;

    use crate::{SerialDevice, TcpDevice};

    #[test]
    fn test_serial_device() {
        // test with serial echo server at serial0
        let mut dev = SerialDevice::create("/tmp/serial0:115200", 1440, |_| 1).unwrap();
        thread::sleep(time::Duration::from_secs(1));
        dev.stop();
    }

    #[test]
    fn test_tcp_device() {
        // test with TCP echo server at port 2000
        let mut dev = TcpDevice::create("127.0.0.1:2000", 1440, |_| 1).unwrap();
        thread::sleep(time::Duration::from_secs(1));
        dev.stop();
    }
}
