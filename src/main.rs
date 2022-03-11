use anyhow::Result;
use clap::Parser;

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

    let listener = workers::create_worker(&args.listener_port)?;
    let sender = workers::create_worker(&args.sender_port)?;
    // output_worker.send();

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
