use anyhow::Result;
use clap::Parser;
use url::Url;

mod workers;

fn url_validator(v: &str) -> Result<(), String> {
    match Url::parse(v) {
        Ok(u) => match u.scheme() {
            "tcpclient" => Ok(()),
            "udp" => Ok(()),
            "serial" => Ok(()),
            _ => Err(String::from("Url scheme not supported")),
        },
        Err(error) => Err(String::from("Url invalid: ") + &error.to_string()),
    }
}

/// A speed and loss rate tester for transparent bridge between tcp, udp and serial port
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Listening port for the testing data
    #[clap(long = "listen-to", validator(url_validator))]
    listen_to_port: Url,

    /// Sending port for the testing data (required when using udp in "listen-to" port)
    #[clap(long = "listen-from")]
    listen_from_port: Option<Url>,

    /// Destination of the testing data
    #[clap(long = "send-to", validator(url_validator))]
    send_to_port: Url,

    /// Sending port for the testing data (required when using udp in "send-to" port)
    #[clap(long = "send-from")]
    send_from_port: Option<Url>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!(
        "Test will send data to {} and listen to {}",
        args.send_to_port, args.listen_to_port
    );

    // let output_worker = workers::create_worker(args.output_port)?;
    // output_worker.send();

    Ok(())
}
