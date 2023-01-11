use anyhow::{Context, Result};
use clap::{Arg, Command};
use rand::distributions::Standard;
use rand::{thread_rng, Rng};
use std::{
    any::type_name,
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

#[derive(Default)]
struct Generator {
    name: String,
    queue: Vec<u8>,
    sync: u8,
}

impl Generator {
    const SYNC: &str = "sync";

    fn create(name: &str) -> Result<Generator> {
        Ok(Generator {
            name: name.to_string(),
            queue: Vec::new(),
            sync: 0
        })
    }

    fn generate(&mut self) -> Vec<u8> {
        // sync step 0: send synchronization string "sync"
        if self.sync == 0 {
            println!("[{}] {} tx: send out sync string", type_name::<Self>(), self.name);
            self.sync = 1;
            Self::SYNC.as_bytes().to_owned()
        } else if self.sync == 1 {
            // waiting for validator to receive the synchronization string
            // validator will set sync to 2 when they are in sync
            println!("[{}] {} tx: wait for validator to receive sync string", type_name::<Self>(), self.name);
            vec![]
        } else {
            let mut rng = thread_rng();
            let data: Vec<u8> = (&mut rng).sample_iter(Standard).take(1_000).collect();
            self.queue.extend(data.iter());
            data
        }
    }

    fn validate(&mut self, data: &[u8]) -> bool {
        if self.sync == 0 {
            println!("[{}] {} rx: sync = 0, unexpected data: {:?}", type_name::<Self>(), self.name, data);
            true
        } else if self.sync == 1 {
            if data.len() >= Self::SYNC.len() && &data[data.len()-Self::SYNC.len()..] == Self::SYNC.as_bytes() {
                println!("[{}] {} rx: sync string received in {:?}", type_name::<Self>(), self.name, data);
                self.sync = 2;
            } else {
                println!("[{}] {} rx: sync = 1, unexpected data: {:?}", type_name::<Self>(), self.name, data);
            }
            true
        } else {
            let reference = self.queue.drain(0..data.len()).collect::<Vec<_>>();
            reference == data
        }
    }
}

struct GenericDevice {
    threads: Vec<JoinHandle<()>>,
}

impl GenericDevice {
    fn create<T: std::io::Read + std::io::Write + std::marker::Send + 'static>(
        mut tx_device: T,
        mut rx_device: T,
        maybe_tx_generator: Option<Arc<Mutex<Generator>>>,
        maybe_rx_generator: Option<Arc<Mutex<Generator>>>,
        stop: Arc<AtomicBool>,
    ) -> Result<GenericDevice> {
        let stop_tx = stop.clone();
        let stop_rx = stop.clone();
        let mut threads = Vec::new();
        // rx
        if let Some(rx_generator) = maybe_rx_generator {
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
        }

        // tx
        if let Some(tx_generator) = maybe_tx_generator {
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
        }

        Ok(GenericDevice { threads })
    }
}

fn create_tcp_device(
    config: &str,
    tx_generator: Option<Arc<Mutex<Generator>>>,
    rx_generator: Option<Arc<Mutex<Generator>>>,
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
    tx_generator: Option<Arc<Mutex<Generator>>>,
    rx_generator: Option<Arc<Mutex<Generator>>>,
    stop: Arc<AtomicBool>,
) -> Result<GenericDevice> {
    let mut serial_iter = config.split(':');
    let device = serial_iter.next().unwrap();
    let baud_rate = serial_iter
        .next()
        .unwrap_or_else(|| {
            println!("Missing baud rate");
            exit(1)
        })
        .parse::<u32>()
        .unwrap_or_else(|e| {
            println!("Invalid baud rate: {:?}", e);
            exit(1)
        });

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
    tx_generator: Option<Arc<Mutex<Generator>>>,
    rx_generator: Option<Arc<Mutex<Generator>>>,
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

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum Mode {
    Normal,
    UpStream,
    DownStream,
    Echo,
}

fn run(
    mode: Mode,
    configs: [&str; 2],
    devices: &mut Vec<GenericDevice>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    println!("Test in {:?} mode", mode);
    match mode {
        Mode::Normal => {
            let upstream = Arc::new(Mutex::new(Generator::create("upstream")?));
            let downstream = Arc::new(Mutex::new(Generator::create("downstream")?));
            devices.push(create_device(
                configs[0],
                Some(upstream.clone()),
                Some(downstream.clone()),
                stop.clone(),
            )?);
            devices.push(create_device(
                configs[1],
                Some(downstream.clone()),
                Some(upstream.clone()),
                stop.clone(),
            )?);
        }
        Mode::UpStream => {
            let generator = Arc::new(Mutex::new(Generator::create("upstream")?));
            devices.push(create_device(
                configs[0],
                Some(generator.clone()),
                None,
                stop.clone(),
            )?);
            devices.push(create_device(
                configs[1],
                None,
                Some(generator.clone()),
                stop.clone(),
            )?);
        }
        Mode::DownStream => {
            let generator = Arc::new(Mutex::new(Generator::create("downstream")?));
            devices.push(create_device(
                configs[0],
                None,
                Some(generator.clone()),
                stop.clone(),
            )?);
            devices.push(create_device(
                configs[1],
                Some(generator.clone()),
                None,
                stop.clone(),
            )?);
        }
        Mode::Echo => {
            let generator = Arc::new(Mutex::new(Generator::create("echo")?));
            devices.push(create_device(
                configs[0],
                Some(generator.clone()),
                Some(generator.clone()),
                stop.clone(),
            )?);
        }
    };

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
                .value_names(["TYPE:DEVICE", "TYPE:DEVICE"])
                .required(true)
                .num_args(2)
                .help("Serial device: serial:/dev/ttyUSB0:115200 (Linux) or serial:COM1:115200 (Windows),\n\
                       TCP server: tcp:192.168.7.1:7\n\
                       Use - as place holder if only one device is supplied in echo mode")
        )
        .arg(
            Arg::new("mode")
                .long("mode")
                .short('m')
                .default_value("normal")
                .value_parser(clap::builder::EnumValueParser::<Mode>::new())
                .help("Select testing mode")
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

    run(
        *m.get_one("mode").unwrap(),
        configs,
        &mut devices,
        stop_signal.clone(),
    )?;
    wait_ctrl_c();
    stop(&mut devices, stop_signal.clone());

    Ok(())
}

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicBool;
    use std::time;
    use std::{sync::Arc, thread};

    use crate::{run, stop, Generator, GenericDevice, Mode};

    /// test data generator/validator
    #[test]
    fn test_generator() {
        let mut gen = Generator::create("").unwrap();
        let data = gen.generate();
        assert!(gen.validate(&data));
    }

    /// test with serial echo server at /tmp/serial0
    #[test]
    fn test_serial_device() {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let mut devices: Vec<GenericDevice> = Vec::new();

        run(
            Mode::Echo,
            ["serial:/tmp/serial0:115200", "-"],
            &mut devices,
            stop_signal.clone(),
        )
        .unwrap();

        thread::sleep(time::Duration::from_secs(1));

        stop(&mut devices, stop_signal);
    }

    /// test with TCP echo server at port 4000
    #[test]
    fn test_tcp_device() {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let mut devices: Vec<GenericDevice> = Vec::new();

        run(
            Mode::Echo,
            ["tcp:127.0.0.1:4000", "-"],
            &mut devices,
            stop_signal.clone(),
        )
        .unwrap();

        thread::sleep(time::Duration::from_secs(1));

        stop(&mut devices, stop_signal);
    }

    /// test with ser2net service between tcp:127.0.0.1:3000 and serial:/tmp/serial1:115200
    #[test]
    fn test_ser2net() {
        {
            let stop_signal = Arc::new(AtomicBool::new(false));
            let mut devices: Vec<GenericDevice> = Vec::new();

            run(
                Mode::Normal,
                ["tcp:127.0.0.1:3000", "serial:/tmp/serial1:115200"],
                &mut devices,
                stop_signal.clone(),
            )
            .unwrap();

            thread::sleep(time::Duration::from_secs(1));

            stop(&mut devices, stop_signal);
        }

        {
            let stop_signal = Arc::new(AtomicBool::new(false));
            let mut devices: Vec<GenericDevice> = Vec::new();

            run(
                Mode::UpStream,
                ["tcp:127.0.0.1:3000", "serial:/tmp/serial1:115200"],
                &mut devices,
                stop_signal.clone(),
            )
            .unwrap();

            thread::sleep(time::Duration::from_secs(1));

            stop(&mut devices, stop_signal);
        }

        {
            let stop_signal = Arc::new(AtomicBool::new(false));
            let mut devices: Vec<GenericDevice> = Vec::new();

            run(
                Mode::DownStream,
                ["tcp:127.0.0.1:3000", "serial:/tmp/serial1:115200"],
                &mut devices,
                stop_signal.clone(),
            )
            .unwrap();

            thread::sleep(time::Duration::from_secs(1));

            stop(&mut devices, stop_signal);
        }
    }
}
