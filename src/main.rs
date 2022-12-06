use anyhow::{anyhow, Result};
use clap::{Arg, Command, Parser, ArgMatches};
use device::Device;
use url::Url;

mod device;
mod serialdevice;
mod udpdevice;

/// A speed and loss rate tester for transparent bridge between tcp, udp and serial port
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// url of device a
    #[clap(long = "device-a", short = 'a')]
    device_a: String,

    /// url of device b
    #[clap(long = "device-b", short = 'b')]
    device_b: String,

    /// number of test packages
    #[clap(long = "package-number", short = 'n', default_value = "1000")]
    package_num: usize,

    /// length of each package
    #[clap(long = "package-length", short = 'p', default_value = "100")]
    package_length: usize,
}

fn create(device_url: Url) -> Result<Box<dyn Device>> {
    if device_url.scheme() == "serial" {
        let list: Vec<&str> = device_url.path().split(':').collect();
        Ok(Box::new(serialdevice::SerialDevice::create(
            list[0],
            list[1].parse()?,
        )?))
    } else if device_url.scheme() == "udp" {
        Ok(Box::new(udpdevice::UdpDevice::create("", "")?))
    } else {
        Err(anyhow!("invalid url"))
    }
}

fn main() -> Result<()> {
    // let args = Args::parse();
    let m = Command::new("TT Tester")
        .author("Han.Liu, liuhan211211@gmail.com")
        .version(clap::crate_version!())
        .about("Speed and package drop tester for transparent transmission between serial port, tcp and udp devices")
        .arg(Arg::new("device").short('d').long("device").help("Specify device"))
        .after_help(
            "Longer explanation to appear after the options when \
                 displaying the help information from --help or -h",
        )
        .get_matches();

    // println!("Device A: {}", args.device_a);
    // let device_a_url = Url::parse(&args.device_a)?;
    // let _a = create(device_a_url)?;

    // println!("Device B: {}", args.device_b);
    // let device_b_url = Url::parse(&args.device_b)?;
    // let _b = create(device_b_url)?;

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
