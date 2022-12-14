use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{self, Duration},
};

use signal_hook::{consts::SIGINT, iterator::Signals};

use anyhow::{Context, Result};
use clap::{Arg, Command};

struct TcpDevice {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl TcpDevice {
    fn create(
        config: &str,
        send_vec: Arc<Mutex<Vec<u8>>>,
        rec_vec: Arc<Mutex<Vec<u8>>>,
    ) -> Result<TcpDevice> {
        let tcp = TcpStream::connect(config)
            .with_context(|| format!("Failed to connect to remote_ip {}", config))?;
        tcp.set_nodelay(true)?; // no write package grouping
        tcp.set_write_timeout(None)?; // blocking write
        tcp.set_read_timeout(Some(time::Duration::from_millis(10)))?; // unblocking read

        let mut tcp_tx = tcp.try_clone()?;
        let mut tcp_rx = tcp.try_clone()?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();

        let mut threads = Vec::new();

        // tcp tx
        threads.push(thread::spawn(move || {
            println!("tcp tx starts");
            loop {
                {
                    let mut vec = send_vec.lock().unwrap();
                    let n = tcp_tx.write(vec.as_ref()).unwrap();
                    assert_eq!(n, vec.len());
                    vec.clear();
                }

                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(1))
            }
            println!("tcp tx stopped");
        }));

        // tcp rx
        threads.push(thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            println!("tcp rx starts");
            loop {
                if let Ok(n) = tcp_rx.read(&mut buf) {
                    let mut vec = rec_vec.lock().unwrap();
                    vec.append(buf[0..n].to_vec().as_mut())
                }

                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(1));
            }
            println!("tcp rx stopped");
        }));

        Ok(TcpDevice { stop, threads })
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        while let Some(t) = self.threads.pop() {
            t.join().unwrap();
        }
    }
}

struct SerialDevice {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl SerialDevice {
    fn create(
        config: &str,
        send_vec: Arc<Mutex<Vec<u8>>>,
        rec_vec: Arc<Mutex<Vec<u8>>>,
    ) -> Result<SerialDevice> {
        let mut serial_iter = config.split(':');
        let device = serial_iter.next().unwrap();
        let baud_rate = serial_iter.next().unwrap().parse::<u32>().unwrap();

        let serialport = serialport::new(device, baud_rate).open().with_context(|| {
            format!(
                "Failed to open serialport device {} with baud rate {}",
                device, baud_rate
            )
        })?;
        let mut serialport_tx = serialport.try_clone()?;
        let mut serialport_rx = serialport.try_clone()?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();

        let mut threads = Vec::new();

        // serial tx
        threads.push(thread::spawn(move || {
            println!("serial tx starts");
            loop {
                {
                    let mut vec = send_vec.lock().unwrap();
                    let n = serialport_tx.write(vec.as_ref()).unwrap();
                    assert_eq!(n, vec.len());
                    vec.clear();
                }

                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(1));
            }
            println!("serial tx stopped");
        }));

        // serial rx
        threads.push(thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            println!("serial rx starts");
            loop {
                if let Ok(n) = serialport_rx.read(&mut buf[..]) {
                    let mut vec = rec_vec.lock().unwrap();
                    vec.append(buf[0..n].to_vec().as_mut())
                }

                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(1));
            }
            println!("serial rx stopped");
        }));

        Ok(SerialDevice { stop, threads })
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        while let Some(t) = self.threads.pop() {
            t.join().unwrap();
        }
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

    let send_buf_1 = Arc::new(Mutex::new(vec![1_u8, 2, 3, 4, 5]));
    let rec_buf_1 = Arc::new(Mutex::new(Vec::<u8>::new()));

    let send_buf_2 = Arc::new(Mutex::new(vec![1_u8, 2, 3, 4, 5]));
    let rec_buf_2 = Arc::new(Mutex::new(Vec::<u8>::new()));

    let mut tcp_device = TcpDevice::create(
        m.get_one::<String>("tcp").expect("tcp config is required"),
        send_buf_1,
        rec_buf_1,
    )?;
    let mut serial_device = SerialDevice::create(
        m.get_one::<String>("serial")
            .expect("serial config is required"),
        send_buf_2,
        rec_buf_2,
    )?;

    let mut signals = Signals::new(&[SIGINT])?;
    signals.wait();

    tcp_device.stop();
    serial_device.stop();

    Ok(())
}

#[cfg(test)]
mod test {
    use std::time;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use crate::{SerialDevice, TcpDevice};

    #[test]
    fn test_serial_device() {
        let send_buf = Arc::new(Mutex::new(vec![1_u8, 2, 3, 4, 5]));
        let rec_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let rec_buf_clone = rec_buf.clone();

        // test with serial echo server at /tmp/serial0
        let mut dev = SerialDevice::create("/tmp/serial0:115200", send_buf, rec_buf).unwrap();
        thread::sleep(time::Duration::from_secs(1));
        assert_eq!(*rec_buf_clone.lock().unwrap(), &[1_u8, 2, 3, 4, 5]);
        dev.stop();
    }

    #[test]
    fn test_tcp_device() {
        let send_buf = Arc::new(Mutex::new(vec![1_u8, 2, 3, 4, 5]));
        let rec_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let rec_buf_clone = rec_buf.clone();

        // test with TCP echo server at port 4000
        let mut dev = TcpDevice::create("127.0.0.1:4000", send_buf, rec_buf).unwrap();
        thread::sleep(time::Duration::from_secs(1));
        assert_eq!(*rec_buf_clone.lock().unwrap(), &[1_u8, 2, 3, 4, 5]);
        dev.stop();
    }

    #[test]
    fn test_tcp_and_serial() {
        let send_buf = Arc::new(Mutex::new(vec![1_u8, 2, 3, 4, 5]));
        let rec_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let rec_buf_clone = rec_buf.clone();

        // tcp <> serial pass through between /tmp/serial1 and port 3000
        let mut tcp =
            TcpDevice::create("127.0.0.1:3000", send_buf, Arc::new(Mutex::new(Vec::new())))
                .unwrap();
        let mut ser = SerialDevice::create(
            "/tmp/serial1:115200",
            Arc::new(Mutex::new(Vec::new())),
            rec_buf,
        )
        .unwrap();
        thread::sleep(time::Duration::from_secs(3));
        assert_eq!(*rec_buf_clone.lock().unwrap(), &[1_u8, 2, 3, 4, 5]);
        tcp.stop();
        ser.stop();
    }
}
