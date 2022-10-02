use anyhow::Result;
use clap::Parser;

mod device;
mod serialdevice;
mod udpdevice;

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
    #[clap(long = "listener", short = 'l', validator(port_validator))]
    listener_port: String,

    /// sender port in format: type,local,remote
    #[clap(long = "sender", short = 's', validator(port_validator))]
    sender_port: String,

    /// number of test packages
    #[clap(long = "package-number", short = 'n', default_value = "1000")]
    package_num: usize,

    /// length of each package
    #[clap(long = "package-length", short = 'p', default_value = "100")]
    package_length: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("listener: {}", args.listener_port);
    println!("sender: {}", args.sender_port);

    // let sender = workers::create_worker(&args.sender_port).unwrap();
    // let listener = workers::create_worker(&args.listener_port).unwrap();

    // println!("Testing...");
    // let start = Instant::now();
    // for i in 0..args.package_num {
    //     let pkg = vec![i as u8; args.package_length];
    //     // println!("[dispatcher_thread]  release pkg: {:?}", pkg.clone());

    //     sender.send(pkg.clone(), None).unwrap();
    //     let received = listener.receive(Some(Duration::from_millis(10))).unwrap();

    //     assert_eq!(received, pkg);
    //     // println!("[dispatcher_thread] pkg verified");
    // }
    // let duration = start.elapsed();
    // println!("Result:");
    // println!(
    //     "   Transferred {} packages ({} bytes) in {} seconds",
    //     args.package_num,
    //     args.package_num * args.package_length,
    //     duration.as_secs_f32()
    // );
    // println!(
    //     "   Pacakges rate: {:.2} package per second",
    //     args.package_num as f64 / duration.as_secs_f64()
    // );
    // println!(
    //     "   Data rate: {:.2} byte per second",
    //     args.package_num as f64 * args.package_length as f64 / duration.as_secs_f64()
    // );

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::port_validator;

    #[test]
    fn test_port_validator_with_valid_port() {
        assert!(port_validator("udp,,").is_ok());
    }

    #[test]
    fn test_port_validator_with_invalid_port() {
        assert!(port_validator("invalid,,").is_err());
    }
}
