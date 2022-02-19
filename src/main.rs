use anyhow::Result;
use clap::Parser;
use url::Url;

mod workers;

/// A speed and loss test for pass-through bridges between tcp, udp and serial port
#[derive(Parser, Debug)]
#[clap(author = "Han Liu <liuhan211211@gmail.com>", version, about)]
struct Args {
    /// The output port in URL format.
    /// Tcp client: "tcpclient://SERVER_ADDR:PORT".
    /// Udp: "udp://SERVER_ADDR:PORT <LOCAL_ADDR:PORT>".
    /// Serial port: "serial://SERIAL_PORT_DEVICE".
    #[clap(
        short = 'o',
        long = "output",
        required = true,
        multiple_values = true,
        max_values = 2
    )]
    output_port: Vec<Url>,

    /// The input port in URL format.
    /// Tcp client: "tcpclient://SERVER_ADDR:PORT".
    /// Udp: "udp://SERVER_ADDR:PORT <LOCAL_ADDR:PORT>".
    /// Serial port: "serial://SERIAL_PORT_DEVICE".
    #[clap(
        short = 'i',
        long = "input",
        required = true,
        multiple_values = true,
        max_values = 2
    )]
    input_port: Vec<Url>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!(
        "Test will send data to {:?} and listen to {:?}",
        args.input_port, args.output_port
    );

    let output_worker = workers::create_worker(args.output_port)?;
    output_worker.send();

    Ok(())
}
