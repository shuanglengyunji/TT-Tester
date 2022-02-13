use clap::Parser;

mod sender;

/// A functional test tool for mini-bridge
#[derive(Parser, Debug)]
#[clap(author = "Han Liu <liuhan211211@gmail.com>", version, about, long_about = None)]
struct Args {
    /// Input port
    #[clap(short, long, required = true)]
    input: String,

    /// Output port
    #[clap(short, long, required = true)]
    output: String,
}

fn main() {
    let args = Args::parse();
    println!(
        "Test will send data to {} and listen to {}",
        args.output, args.input
    );

    let sender = sender::create_sender(args.output).unwrap();
    sender.output();
}
