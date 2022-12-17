use std::{
    any::type_name,
    collections::VecDeque,
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

struct Controller {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    tx: Arc<Mutex<VecDeque<u8>>>,
    rx: Arc<Mutex<VecDeque<u8>>>,
}

impl Controller {
    fn create() -> Result<Controller> {
        let tx = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let rx = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let tx_clone = tx.clone();
        let rx_clone = rx.clone();

        let sync = Arc::new(AtomicBool::new(false));
        let sync_clone = sync.clone();

        let bridge = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let bridge_clone = bridge.clone();

        let stop = Arc::new(AtomicBool::new(false));
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();

        let mut threads = Vec::new();

        threads.push(thread::spawn(move || {
            println!("controller tx starts");

            // sync data
            let mut index = 0;
            loop {
                if sync.load(Ordering::SeqCst) {
                    break;
                }
                println!("controller tx try sync with value {:?}!", index);
                tx_clone.lock().unwrap().push_back(index);
                bridge.lock().unwrap().push_back(index);
                (index, _) = index.overflowing_add(1);

                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(500));
            }

            // test speed
            loop {
                let new = [index; 100];
                tx_clone.lock().unwrap().extend(new.iter());
                new.iter().for_each(|x| {
                    bridge.lock().unwrap().push_back(*x);
                });
                (index, _) = index.overflowing_add(1);

                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }
            println!("controller tx stopped");
        }));

        threads.push(thread::spawn(move || {
            println!("controller rx starts");
            // sync data
            let first = loop {
                // wait for data
                if let Some(x) = rx_clone.lock().unwrap().pop_front() {
                    println!("controller rx received value {:?}!", x);
                    break x;
                } else {
                    thread::sleep(time::Duration::from_millis(1));
                }
                if stop_rx.load(Ordering::SeqCst) {
                    break 0;
                }
            };
            loop {
                let mut bridge = bridge_clone.lock().unwrap();
                if let Some(x) = bridge.pop_front() {
                    if first == x {
                        println!("controller rx sync at value {:?}!", first);
                        sync_clone.store(true, Ordering::SeqCst);
                        break;
                    } else {
                        println!(
                            "controller with value {:?} rx missed sync value {:?}!",
                            first, x
                        );
                    }
                }
                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(1));
            }

            // test speed
            let mut bytes = 0;
            let mut begin = time::SystemTime::now();
            loop {
                {
                    let mut data_received = rx_clone.lock().unwrap();
                    while let Some(received) = data_received.pop_front() {
                        let expected = loop {
                            if let Some(x) = bridge_clone.lock().unwrap().pop_front() {
                                break x;
                            }
                            thread::sleep(time::Duration::from_millis(1));
                        };
                        if expected != received {
                            panic!("expected: {:?} received: {:?}", expected, received);
                        }
                        bytes = bytes + 1;
                    }
                }
                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                if begin.elapsed().unwrap() >= time::Duration::from_secs(1) {
                    println!("transmission speed: {:?}KB/s", (bytes as f64) / 1000.0);
                    bytes = 0;
                    begin = time::SystemTime::now();
                }
                thread::sleep(Duration::from_millis(1));
            }
            println!("controller rx stopped");
        }));

        Ok(Controller {
            stop,
            threads,
            tx,
            rx,
        })
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        while let Some(t) = self.threads.pop() {
            t.join().unwrap();
        }
    }

    fn tx(&self) -> Arc<Mutex<VecDeque<u8>>> {
        self.tx.clone()
    }

    fn rx(&self) -> Arc<Mutex<VecDeque<u8>>> {
        self.rx.clone()
    }
}

struct GenericDevice {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl GenericDevice {
    fn create<T: std::io::Read + std::io::Write + std::marker::Send + 'static>(
        mut tx_device: T,
        mut rx_device: T,
        tx: Arc<Mutex<VecDeque<u8>>>,
        rx: Arc<Mutex<VecDeque<u8>>>,
    ) -> Result<GenericDevice> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();

        let mut threads = Vec::new();

        // tx
        threads.push(thread::spawn(move || {
            println!("starts tx with device type {}", type_name::<T>());
            loop {
                {
                    let mut vec = tx.lock().unwrap();
                    vec.make_contiguous();
                    let (slice, _) = vec.as_slices(); // we can now be sure that `slice` contains all elements of the deque, while still having immutable access to `buf`.
                    tx_device.write_all(slice).unwrap();
                    vec.clear();
                }

                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(1))
            }
            println!("stops tx with device type {}", type_name::<T>());
        }));

        // rx
        threads.push(thread::spawn(move || {
            let mut buf = [0u8; 2048]; // max 2k
            println!("starts rx with device type {}", type_name::<T>());
            loop {
                if let Ok(n) = rx_device.read(&mut buf) {
                    let mut vec = rx.lock().unwrap();
                    vec.extend(buf[0..n].iter());
                }

                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(1));
            }
            println!("stops rx with device type {}", type_name::<T>());
        }));

        Ok(GenericDevice { stop, threads })
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        while let Some(t) = self.threads.pop() {
            t.join().unwrap();
        }
    }
}

struct TcpDevice {
    device: GenericDevice,
}

impl TcpDevice {
    fn create(
        config: &str,
        tx: Arc<Mutex<VecDeque<u8>>>,
        rx: Arc<Mutex<VecDeque<u8>>>,
    ) -> Result<TcpDevice> {
        let tcp = TcpStream::connect(config)
            .with_context(|| format!("Failed to connect to remote_ip {}", config))?;
        tcp.set_nodelay(true)?; // no write package grouping
        tcp.set_write_timeout(None)?; // blocking write
        tcp.set_read_timeout(Some(time::Duration::from_millis(10)))?; // unblocking read

        Ok(TcpDevice {
            device: GenericDevice::create(tcp.try_clone()?, tcp.try_clone()?, tx, rx)?,
        })
    }

    fn stop(&mut self) {
        self.device.stop();
    }
}

struct SerialDevice {
    device: GenericDevice,
}

impl SerialDevice {
    fn create(
        config: &str,
        tx: Arc<Mutex<VecDeque<u8>>>,
        rx: Arc<Mutex<VecDeque<u8>>>,
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

        Ok(SerialDevice {
            device: GenericDevice::create(
                serialport.try_clone()?,
                serialport.try_clone()?,
                tx,
                rx,
            )?,
        })
    }

    fn stop(&mut self) {
        self.device.stop();
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

    let mut tcp_to_serial_controller = Controller::create().unwrap();
    let mut serial_to_tcp_controller = Controller::create().unwrap();

    let mut tcp_device = TcpDevice::create(
        m.get_one::<String>("tcp").expect("tcp config is required"),
        tcp_to_serial_controller.tx(),
        serial_to_tcp_controller.rx(),
    )?;
    let mut serial_device = SerialDevice::create(
        m.get_one::<String>("serial")
            .expect("serial config is required"),
        serial_to_tcp_controller.tx(),
        tcp_to_serial_controller.rx(),
    )?;

    let mut signals = Signals::new(&[SIGINT])?;
    signals.wait();

    tcp_to_serial_controller.stop();
    serial_to_tcp_controller.stop();
    tcp_device.stop();
    serial_device.stop();

    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::VecDeque;
    use std::time;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use crate::{Controller, SerialDevice, TcpDevice};

    #[test]
    fn test_controller() {
        let mut controller = Controller::create().unwrap();
        let tx = controller.tx();
        let rx = controller.rx();

        let start = time::SystemTime::now();
        let mut drop = 5; // drop first 5 bytes to simulate network delay
        while start.elapsed().unwrap() < time::Duration::from_secs(5) {
            {
                let mut tx_data = tx.lock().unwrap();

                while let Some(x) = tx_data.pop_front() {
                    if drop > 0 {
                        drop = drop - 1;
                        continue;
                    }
                    rx.lock().unwrap().push_back(x);
                }
            }
            thread::sleep(time::Duration::from_millis(20));
        }

        controller.stop();
    }

    #[test]
    fn test_serial_device() {
        let send_buf = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let rec_buf = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let send_buf_clone = rec_buf.clone();
        let rec_buf_clone = rec_buf.clone();

        // test with serial echo server at /tmp/serial0
        let mut dev = SerialDevice::create("/tmp/serial0:115200", send_buf, rec_buf).unwrap();

        send_buf_clone
            .lock()
            .unwrap()
            .extend([1_u8, 2, 3, 4, 5].iter());
        thread::sleep(time::Duration::from_secs(1));
        assert_eq!(*rec_buf_clone.lock().unwrap(), &[1_u8, 2, 3, 4, 5]);

        dev.stop();
    }

    #[test]
    fn test_tcp_device() {
        let send_buf = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let rec_buf = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let send_buf_clone = rec_buf.clone();
        let rec_buf_clone = rec_buf.clone();

        // test with TCP echo server at port 4000
        let mut dev = TcpDevice::create("127.0.0.1:4000", send_buf, rec_buf).unwrap();

        send_buf_clone
            .lock()
            .unwrap()
            .extend([1_u8, 2, 3, 4, 5].iter());
        thread::sleep(time::Duration::from_secs(1));
        assert_eq!(*rec_buf_clone.lock().unwrap(), &[1_u8, 2, 3, 4, 5]);

        dev.stop();
    }
}
