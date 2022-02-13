use std::io::Result;
use std::net::UdpSocket;
use url::{Position, Url};

pub trait Sender {
    fn output(&self);
}

struct UDPSender {
    dest: String,
    udp_socket: UdpSocket,
}

impl Sender for UDPSender {
    fn output(&self) {
        let buf = [1; 10];
        let size = self.udp_socket.send_to(&buf, &self.dest).unwrap();
        println!("sent {} bytes to {}", size, self.dest);
    }
}

fn create_udpsender(dest: String) -> UDPSender {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    UDPSender {
        dest: dest,
        udp_socket: socket,
    }
}

struct SerialPortSender {
    dest_port: String,
}

impl Sender for SerialPortSender {
    fn output(&self) {
        println!("Send to {}", self.dest_port);
    }
}

pub fn create_sender(dest: String) -> Result<Box<dyn Sender>> {
    let url = Url::parse(&dest).unwrap();

    match url.scheme() {
        "udp" => Ok(Box::new(create_udpsender(
            url[Position::BeforeHost..].to_string(),
        ))),
        "serial" => Ok(Box::new(SerialPortSender { dest_port: dest })),
        _ => panic!("No sender found"),
    }
}
