use anyhow::{Context, Result};
use clap::{Arg, Command};
use rand::distributions::Standard;
use rand::{thread_rng, Rng};
use std::{
    any::type_name,
    collections::VecDeque,
    net::TcpStream,
    process::exit,
    result::Result::Ok,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{self, Duration},
};

struct Generator {
    queue: VecDeque<u8>,
}

impl Generator {
    fn create() -> Result<Generator> {
        Ok(Generator {
            queue: VecDeque::new(),
        })
    }

    fn generate(&mut self) -> Vec<u8> {
        let mut rng = thread_rng();
        let data: Vec<u8> = (&mut rng).sample_iter(Standard).take(1_000).collect();
        self.queue.extend(data.iter());
        data
    }

    fn validate(&mut self, data: &[u8]) -> bool {
        let reference = self.queue.drain(0..data.len()).collect::<Vec<_>>();
        reference == data
    }
}

struct GenericDevice {
    threads: Vec<JoinHandle<()>>,
}

impl GenericDevice {
    fn create<T: std::io::Read + std::io::Write + std::marker::Send + 'static>(
        mut tx_device: T,
        mut rx_device: T,
        tx_generator: Arc<Mutex<Generator>>,
        rx_generator: Arc<Mutex<Generator>>,
        stop: Arc<AtomicBool>,
    ) -> Result<GenericDevice> {
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();
        let mut threads = Vec::new();

        // tx
        threads.push(thread::spawn(move || {
            println!("starts tx with device type {}", type_name::<T>());
            loop {
                let data = tx_generator.lock().unwrap().generate();
                tx_device.write_all(&data).unwrap_or_else(|e| {
                    println!("Tx error: {:?}", e);
                    exit(1);
                });
                if stop_tx.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(1))
            }
            println!("stops tx with device type {}", type_name::<T>());
        }));

        // rx

        threads.push(thread::spawn(move || {
            let mut bytes = 0;
            let mut begin = time::SystemTime::now();
            let mut buf = [0u8; 2048]; // max 2k
            println!("starts rx with device type {}", type_name::<T>());
            loop {
                if let Ok(n) = rx_device.read(&mut buf) {
                    if !rx_generator.lock().unwrap().validate(&buf[0..n]) {
                        println!("data mismatch");
                        exit(1);
                    }
                    bytes = bytes + n;
                }
                if stop_rx.load(Ordering::SeqCst) {
                    break;
                }
                if begin.elapsed().unwrap() >= time::Duration::from_secs(1) {
                    println!("transmission speed: {:?}KB/s", (bytes as f64) / 1000.0);
                    bytes = 0;
                    begin = time::SystemTime::now();
                }
                thread::sleep(time::Duration::from_millis(1));
            }
            println!("stops rx with device type {}", type_name::<T>());
        }));

        Ok(GenericDevice { threads })
    }
}

fn create_tcp_device(
    config: &str,
    tx_generator: Arc<Mutex<Generator>>,
    rx_generator: Arc<Mutex<Generator>>,
    stop: Arc<AtomicBool>,
) -> Result<GenericDevice> {
    let tcp = TcpStream::connect(config)
        .with_context(|| format!("Failed to connect to remote_ip {}", config))?;
    tcp.set_nodelay(true)?; // turn off write package grouping, send out tcp package as-is
    tcp.set_write_timeout(Some(time::Duration::from_secs(10)))?; // non-blocking write
    tcp.set_read_timeout(Some(time::Duration::from_millis(10)))?; // non-blocking read

    Ok(GenericDevice::create(
        tcp.try_clone()?,
        tcp.try_clone()?,
        tx_generator,
        rx_generator,
        stop,
    )?)
}

fn create_serial_device(
    config: &str,
    tx_generator: Arc<Mutex<Generator>>,
    rx_generator: Arc<Mutex<Generator>>,
    stop: Arc<AtomicBool>,
) -> Result<GenericDevice> {
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
        tx_generator,
        rx_generator,
        stop,
    )?)
}

fn create_device(
    config: &str,
    tx_generator: Arc<Mutex<Generator>>,
    rx_generator: Arc<Mutex<Generator>>,
    stop: Arc<AtomicBool>,
) -> Result<GenericDevice> {
    if config.starts_with("tcp:") {
        create_tcp_device(&config[4..], tx_generator, rx_generator, stop)
    } else if config.starts_with("serial:") {
        create_serial_device(&config[7..], tx_generator, rx_generator, stop)
    } else {
        panic!("unsupported device {:?}", config)
    }
}

fn run(configs: [&str; 2], devices: &mut Vec<GenericDevice>, stop: Arc<AtomicBool>) -> Result<()> {
    if configs[1] == "echo" {
        // echo mode
        let generator = Arc::new(Mutex::new(Generator::create()?));
        devices.push(create_device(
            configs[0],
            generator.clone(),
            generator.clone(),
            stop.clone(),
        )?);
    } else {
        let generator_1 = Arc::new(Mutex::new(Generator::create()?));
        let generator_2 = Arc::new(Mutex::new(Generator::create()?));
        devices.push(create_device(
            configs[0],
            generator_1.clone(),
            generator_2.clone(),
            stop.clone(),
        )?);
        devices.push(create_device(
            configs[1],
            generator_2.clone(),
            generator_1,
            stop.clone(),
        )?);
    }
    Ok(())
}

fn wait_ctrl_c() {
    let (sender, receiver) = channel();
    ctrlc::set_handler(move || {
        let _ = sender.send(());
    })
    .unwrap();
    receiver.recv().unwrap();
    println!("Goodbye!");
}

fn stop(devices: &mut Vec<GenericDevice>, stop: Arc<AtomicBool>) {
    stop.store(true, Ordering::SeqCst);
    devices.iter_mut().for_each(|d: &mut GenericDevice| {
        while let Some(t) = d.threads.pop() {
            t.join().unwrap();
        }
    });
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

    let configs: [&str; 2] = m
        .get_many::<String>("device")
        .unwrap_or_default()
        .map(|v| v.as_str())
        .collect::<Vec<_>>()
        .as_slice()
        .try_into()
        .unwrap();

    let stop_signal = Arc::new(AtomicBool::new(false));
    let mut devices: Vec<GenericDevice> = Vec::new();

    run(configs, &mut devices, stop_signal.clone())?;
    wait_ctrl_c();
    stop(&mut devices, stop_signal.clone());

    Ok(())
}

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicBool;
    use std::time;
    use std::{sync::Arc, thread};

    use crate::{run, Generator, GenericDevice};

    /// test data generator/validator
    #[test]
    fn test_generator() {
        let mut generate = Generator::create().unwrap();
        let data = generate.generate();
        assert!(generate.validate(&data));
    }

    /// test with serial echo server at /tmp/serial0
    #[test]
    fn test_serial_device() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut devices: Vec<GenericDevice> = Vec::new();

        run(
            ["serial:/tmp/serial0:115200", "echo"],
            &mut devices,
            stop.clone(),
        )
        .unwrap();

        thread::sleep(time::Duration::from_secs(1));

        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        devices.iter_mut().for_each(|d: &mut GenericDevice| {
            while let Some(t) = d.threads.pop() {
                t.join().unwrap();
            }
        });
    }

    /// test with TCP echo server at port 4000
    #[test]
    fn test_tcp_device() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut devices: Vec<GenericDevice> = Vec::new();

        run(["tcp:127.0.0.1:4000", "echo"], &mut devices, stop.clone()).unwrap();

        thread::sleep(time::Duration::from_secs(1));

        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        devices.iter_mut().for_each(|d: &mut GenericDevice| {
            while let Some(t) = d.threads.pop() {
                t.join().unwrap();
            }
        });
    }
}
