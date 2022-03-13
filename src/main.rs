use anyhow::Result;
use clap::Parser;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

mod udpworker;
mod serialworker;
mod workers;

fn port_validator(v: &str) -> Result<(), String> {
    if v.starts_with("tcp") || v.starts_with("udp") || v.starts_with("serial") {
        Ok(())
    } else {
        Err(String::from("Port invalid"))
    }
}

/// A speed and loss rate tester for transparent bridge between tcp, udp and serial port
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// listener port in format: type,local,remote
    #[clap(long = "listener", validator(port_validator))]
    listener_port: String,

    /// sender port in format: type,local,remote
    #[clap(long = "sender", validator(port_validator))]
    sender_port: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("listener: {}", args.listener_port);
    println!("sender: {}", args.sender_port);

    let (sender_tx, sender_rx) = channel();
    let (listener_tx, listener_rx) = channel();

    let sender_thread = thread::spawn(move || {
        let sender = workers::create_worker(&args.sender_port).unwrap();
        loop {
            let pkg: Vec<u8> = sender_rx.recv().unwrap();
            sender.send(pkg.clone(), None).unwrap();
            println!("[sender_thread] sent: {:?}", pkg);
        }
    });

    let listener_thread = thread::spawn(move || {
        let listener = workers::create_worker(&args.listener_port).unwrap();
        loop {
            let received = listener.receive(None).unwrap();
            println!("[listener_thread] received: {:?}", received);
            listener_tx.send(received).unwrap();
        }
    });

    let dispatcher_thread = thread::spawn(move || loop {
        let pkg = vec![1u8, 2, 3, 4];
        println!("[dispatcher_thread]  release pkg: {:?}", pkg.clone());

        sender_tx.send(pkg.clone()).unwrap();
        let received = listener_rx.recv_timeout(Duration::from_millis(10)).unwrap();

        assert_eq!(received, pkg);
        println!("[dispatcher_thread] pkg verified");

        thread::sleep(Duration::from_millis(10));
    });

    sender_thread
        .join()
        .expect("The sender_thread thread has panicked");
    listener_thread
        .join()
        .expect("The listener_thread thread has panicked");
    dispatcher_thread
        .join()
        .expect("The dispatcher_thread thread has panicked");

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{port_validator, workers::create_worker};

    #[test]
    fn test_create_worker() {
        assert!(create_worker("udp,127.0.0.1:8000,127.0.0.1:8001").is_ok());
    }

    #[test]
    fn test_port_validator_with_valid_port() {
        assert!(port_validator("udp,,").is_ok());
    }

    #[test]
    fn test_port_validator_with_invalid_port() {
        assert!(port_validator("invalid,,").is_err());
    }
}
