use std::{
    any::type_name,
    collections::VecDeque,
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{self, Duration},
};

use anyhow::{Context, Result};
use clap::{Arg, Command};

struct Controller {
    threads: Vec<JoinHandle<()>>,
}

impl Controller {
    fn create(
        tx: Arc<Mutex<VecDeque<u8>>>,
        rx: Arc<Mutex<VecDeque<u8>>>,
        stop: Arc<AtomicBool>,
    ) -> Result<Controller> {
        let sync = Arc::new(AtomicBool::new(false));
        let sync_clone = sync.clone();

        let bridge = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let bridge_clone = bridge.clone();

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
                tx.lock().unwrap().push_back(index);
                bridge.lock().unwrap().push_back(index);
                (index, _) = index.overflowing_add(1);

                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(500));
            }

            // test speed
            loop {
                let new = [index; 1_000];
                tx.lock().unwrap().extend(new.iter());
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
            loop {
                // wait for data
                if let Some(x) = rx.lock().unwrap().pop_front() {
                    println!("controller rx received value {:?}!", x);
                    let mut bridge_mutex = bridge_clone.lock().unwrap();
                    if let Some(n) = bridge_mutex.iter().position(|y| x == *y) {
                        sync_clone.store(true, Ordering::SeqCst);
                        bridge_mutex.drain(0..=n);
                        println!("controller rx sync at value {:?}!", x);
                        break;
                    } else {
                        println!(
                            "controller rx received value {:?} doesn't match sync value {:?}",
                            x, bridge_mutex
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
                    let mut data_received = rx.lock().unwrap();
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

        Ok(Controller { threads })
    }
}

struct GenericDevice {
    threads: Vec<JoinHandle<()>>,
    tx: Arc<Mutex<VecDeque<u8>>>,
    rx: Arc<Mutex<VecDeque<u8>>>,
}

impl GenericDevice {
    fn create<T: std::io::Read + std::io::Write + std::marker::Send + 'static>(
        mut tx_device: T,
        mut rx_device: T,
        stop: Arc<AtomicBool>,
    ) -> Result<GenericDevice> {
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();

        let mut threads = Vec::new();

        let tx = Arc::new(Mutex::new(VecDeque::new()));
        let rx = Arc::new(Mutex::new(VecDeque::new()));
        let tx_clone = tx.clone();
        let rx_clone = rx.clone();

        // tx
        threads.push(thread::spawn(move || {
            println!("starts tx with device type {}", type_name::<T>());
            loop {
                {
                    let mut vec = tx_clone.lock().unwrap();
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
                    let mut vec = rx_clone.lock().expect(&format!(
                        "Failed to acquire rx lock for {:?}",
                        type_name::<T>()
                    ));
                    vec.extend(buf[0..n].iter());
                }

                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(time::Duration::from_millis(1));
            }
            println!("stops rx with device type {}", type_name::<T>());
        }));

        Ok(GenericDevice { threads, tx, rx })
    }
}

fn create_tcp_device(config: &str, stop: Arc<AtomicBool>) -> Result<GenericDevice> {
    let tcp = TcpStream::connect(config)
        .with_context(|| format!("Failed to connect to remote_ip {}", config))?;
    tcp.set_nodelay(false)?; // use write package grouping
    tcp.set_write_timeout(None)?; // blocking write
    tcp.set_read_timeout(Some(time::Duration::from_millis(10)))?; // unblocking read

    Ok(GenericDevice::create(
        tcp.try_clone()?,
        tcp.try_clone()?,
        stop,
    )?)
}

fn create_serial_device(config: &str, stop: Arc<AtomicBool>) -> Result<GenericDevice> {
    let mut serial_iter = config.split(':');
    let device = serial_iter.next().unwrap();
    let baud_rate = serial_iter.next().unwrap().parse::<u32>().unwrap();

    let mut serialport = serialport::new(device, baud_rate).open().with_context(|| {
        format!(
            "Failed to open serialport device {} with baud rate {}",
            device, baud_rate
        )
    })?;
    serialport
        .set_timeout(time::Duration::from_secs(1))
        .unwrap();

    Ok(GenericDevice::create(
        serialport.try_clone()?,
        serialport.try_clone()?,
        stop,
    )?)
}

fn create_device(config: &str, stop: Arc<AtomicBool>) -> Result<GenericDevice> {
    if config.starts_with("tcp:") {
        create_tcp_device(&config[4..], stop)
    } else if config.starts_with("serial:") {
        create_serial_device(&config[7..], stop)
    } else {
        panic!("unsupported device {:?}", config)
    }
}

fn main() -> Result<()> {
    let m = Command::new("ser2tcp-tester")
        .version(clap::crate_version!())
        .about("Speed tester for transparent transmission between tcp and serial port")
        .arg(
            Arg::new("device")
                .required(true)
                .short('d').long("device")
                .value_names(["TYPE:DEVICE", "TYPE:DEVICE or echo"])
                .num_args(2)
                .help("Serial port: serial:/dev/ttyUSB0:115200 (Linux) or serial:COM1:115200 (Windows),\n\
                       TCP: tcp:192.168.7.1:8000 for tcp server\n\
                       Echo mode: use \"echo\" in place of the second device"),
        )
        .get_matches();

    let configs = m
        .get_many::<String>("device")
        .unwrap_or_default()
        .map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert_eq!(configs.len(), 2);

    let stop = Arc::new(AtomicBool::new(false));
    let mut device_vec: Vec<GenericDevice> = Vec::new();
    let mut controller_vec: Vec<Controller> = Vec::new();

    if configs[1] == "echo" {
        // echo mode
        device_vec.push(create_device(configs[0], stop.clone())?);
        controller_vec.push(Controller::create(
            device_vec[0].tx.clone(),
            device_vec[0].rx.clone(),
            stop.clone(),
        )?);
    } else {
        device_vec.push(create_device(configs[0], stop.clone())?);
        device_vec.push(create_device(configs[1], stop.clone())?);
        controller_vec.push(Controller::create(
            device_vec[0].tx.clone(),
            device_vec[1].rx.clone(),
            stop.clone(),
        )?);
        controller_vec.push(Controller::create(
            device_vec[1].tx.clone(),
            device_vec[0].rx.clone(),
            stop.clone(),
        )?);
    }

    // wait for ctrl-c
    let (sender, receiver) = channel();
    ctrlc::set_handler(move || {
        let _ = sender.send(());
    })?;
    receiver.recv()?;
    println!("Goodbye!");

    stop.store(true, Ordering::SeqCst);
    controller_vec.iter_mut().for_each(|d: &mut Controller| {
        while let Some(t) = d.threads.pop() {
            t.join().unwrap();
        }
    });
    device_vec.iter_mut().for_each(|d: &mut GenericDevice| {
        while let Some(t) = d.threads.pop() {
            t.join().unwrap();
        }
    });

    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;
    use std::time;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use crate::{
        create_device, create_serial_device, create_tcp_device, Controller, GenericDevice,
    };

    #[test]
    fn test_controller() {
        let tx = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let rx = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let tx_clone = tx.clone();
        let rx_clone = rx.clone();
        let stop = Arc::new(AtomicBool::new(false));

        let mut controller = Controller::create(tx, rx, stop.clone()).unwrap();

        let start = time::SystemTime::now();
        let mut drop = 5; // drop first 5 bytes to simulate network delay
        while start.elapsed().unwrap() < time::Duration::from_secs(5) {
            {
                let mut tx_data = tx_clone.lock().unwrap();

                while let Some(x) = tx_data.pop_front() {
                    if drop > 0 {
                        drop = drop - 1;
                        continue;
                    }
                    rx_clone.lock().unwrap().push_back(x);
                }
            }
            thread::sleep(time::Duration::from_millis(20));
        }

        stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn test_serial_device() {
        let stop = Arc::new(AtomicBool::new(false));

        // test with serial echo server at /tmp/serial0
        let mut dev = create_serial_device("/tmp/serial0:115200", stop.clone()).unwrap();
        dev.tx
            .clone()
            .lock()
            .unwrap()
            .extend([1_u8, 2, 3, 4, 5].iter());

        thread::sleep(time::Duration::from_secs(1));

        assert_eq!(*dev.rx.clone().lock().unwrap(), &[1_u8, 2, 3, 4, 5]);
    }

    #[test]
    fn test_tcp_device() {
        let stop = Arc::new(AtomicBool::new(false));

        // test with TCP echo server at port 4000
        let mut dev = create_tcp_device("127.0.0.1:4000", stop.clone()).unwrap();
        dev.tx
            .clone()
            .lock()
            .unwrap()
            .extend([1_u8, 2, 3, 4, 5].iter());

        thread::sleep(time::Duration::from_secs(1));

        assert_eq!(*dev.rx.clone().lock().unwrap(), &[1_u8, 2, 3, 4, 5]);
    }
}
