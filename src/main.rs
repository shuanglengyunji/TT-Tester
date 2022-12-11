use core::time;
use std::{
    io::{Read, Write},
    net::TcpStream,
    string, thread,
};

use signal_hook::{consts::SIGINT, iterator::Signals};

use anyhow::{anyhow, Context, Result};
use bus::Bus;
use clap::{Arg, Command};

fn main() -> Result<()> {
    // let args = Args::parse();
    let m = Command::new("ser2tcp-tester")
        .author("Han.Liu, liuhan211211@gmail.com")
        .version(clap::crate_version!())
        .about("Speed tester for transparent transmission between tcp and serial port")
        // .arg(
        //     Arg::new("serial")
        //         .required(true)
        //         .short('s')
        //         .long("serial")
        //         .value_name("DEVICE:BAUD_RATE")
        //         .help("Serial port device, for example: /dev/ttyUSB0:115200 (Linux) or COM1:115200 (Windows)"),
        // )
        // .arg(
        //     Arg::new("tcp")
        //         .short('t')
        //         .required(true)
        //         .long("tcp")
        //         .value_name("ADDRESS:PORT")
        //         .help("Tcp port, for example: 192.168.7.1:8000"),
        // )
        .get_matches();

    let mut stop_signal: Bus<u8> = Bus::new(1);
    let mut stop_signal_reader_1 = stop_signal.add_rx();
    let mut stop_signal_reader_2 = stop_signal.add_rx();
    let mut stop_signal_reader_3 = stop_signal.add_rx();
    let mut stop_signal_reader_4 = stop_signal.add_rx();

    //#######################################################################################
    //                                        TCP
    //#######################################################################################

    let remote_ip = "127.0.0.1:2000";
    let tcp = TcpStream::connect(remote_ip)
        .with_context(|| format!("Failed to connect to remote_ip {}", remote_ip))?;
    tcp.set_nodelay(true)?; // no write package grouping
    tcp.set_write_timeout(None)?; // blocking write
    tcp.set_read_timeout(None)?; // blocking read

    let mut tcp_tx = tcp.try_clone()?;
    let mut tcp_rx = tcp.try_clone()?;

    // tcp tx
    let handle_tcp_tx = thread::spawn(move || {
        println!("tcp tx starts");
        loop {
            if let Ok(size) = tcp_tx.write(b"test") {
                // assert_eq!(size, 1);
            } else {
                assert!(false);
            }
            if stop_signal_reader_1.try_recv().is_ok() {
                break;
            }
            thread::sleep(time::Duration::from_millis(10));
        }
        println!("tcp tx stopped");
    });

    // // tcp rx
    // thread::spawn(move || {
    //     println!("tcp rx starts");
    //     {
    //         let mut buf = [0u8; 2048]; // max 2k
    //         loop {
    //             if let Ok(size) = tcp_rx.read(&mut buf) {
    //                 for i in 0..size {
    //                     assert_eq!(buf[i], 1); // validate
    //                 }
    //             } else {
    //                 assert!(false); // error, should not goes here
    //             }
    //             if stop_signal_reader_2.try_recv().is_ok() {
    //                 break;
    //             }
    //         }
    //     }
    //     println!("tcp rx stopped");
    // });

    //#######################################################################################
    //                                        SERIAL
    //#######################################################################################

    let device = "/tmp/serial2";
    let baud_rate = 115200;
    let serialport = serialport::new(device, baud_rate).open().with_context(|| {
        format!(
            "Failed to open serialport device {} with baud rate {}",
            device, baud_rate
        )
    })?;
    let mut serialport_tx = serialport.try_clone()?;
    let mut serialport_rx = serialport.try_clone()?;

    // // serial tx
    // thread::spawn(move || loop {
    //     // if let Ok(pkg) = sender_rx.try_recv() {
    //     //     serialport_tx.write(&pkg).unwrap();
    //     // }
    //     if stop_signal_reader_3.try_recv().is_ok() {
    //         break;
    //     }
    // });

    // serial rx
    let handle_serial_rx = thread::spawn(move || {
        let mut buf = [0u8; 2048]; // max 2k

        println!("serial rx starts");
        loop {
            // if let Ok(size) = serialport_rx.read(&mut buf) {
            //     receiver_tx.send(buf[..size].to_vec()).unwrap();
            // }
            if let Ok(n) = serialport_rx.read(&mut buf[..]) {
                println!("Received: {:?}", std::str::from_utf8(&buf[..n]));
            }

            if stop_signal_reader_4.try_recv().is_ok() {
                break;
            }
            thread::sleep(time::Duration::from_millis(10));
        }
        println!("serial rx stopped");
    });

    let mut signals = Signals::new(&[SIGINT])?;
    for sig in signals.forever() {
        println!("\n===== Received signal {:?} =====", sig);
        break;
    }

    stop_signal.broadcast(0);
    handle_tcp_tx.join().unwrap();
    handle_serial_rx.join().unwrap();

    Ok(())
}

#[cfg(test)]
mod test {}
