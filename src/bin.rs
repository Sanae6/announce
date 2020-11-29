use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::str::FromStr;

use announce_au::{Data, HazelMessage, read_packet, write_packet};

use crate::config::Config;

pub(crate) mod config {
    use std::fs::read_to_string;
    use std::io;

    use serde::Deserialize;

    #[derive(Deserialize, Clone)]
    pub struct Config {
        pub endpoint: Endpoint,
        pub message: Message,
    }

    #[derive(Deserialize, Clone)]
    pub struct Endpoint {
        pub ip: String,
        pub port: Option<u16>,
    }

    #[derive(Deserialize, Clone)]
    pub struct Message {
        pub unique_id: u32,
        pub default_message: String,
        pub message: [String; 5],
    }

    pub fn load_toml() -> io::Result<Config> {
        let str = read_to_string("./config.toml")?;
        Ok(toml::from_str(str.as_str()).expect("Failed to deserialize the config."))
    }
}

struct Client {
    nonce: u16,
    id: u32,
}

struct Server {
    clients: HashMap<SocketAddr, Client>,
    client_deletion: Vec<SocketAddr>,
    receive_buffer: Cursor<Vec<u8>>,
    socket: UdpSocket,
    config: Config,
}

fn message_received(_length: usize, address: SocketAddr, state: &mut Server) -> io::Result<()> {
    let clients = state.clients.borrow_mut();
    let mut client = clients.entry(address).or_insert(Client {
        nonce: 0,
        id: 0,
    });
    let mut buffer = Cursor::new(Vec::new());
    state.receive_buffer.set_position(0);
    let packet = read_packet(&mut state.receive_buffer)?;
    if let HazelMessage::Reliable((nonce, _)) |
    HazelMessage::Hello((nonce, _)) |
    HazelMessage::Ping(nonce) = packet {
        write_packet(HazelMessage::Ack(nonce), &mut buffer)?;
        state.socket.send_to(buffer.get_mut(), address)?;
        buffer.set_position(0);
    }

    //println!("read {:02x?}", &state.receive_buffer.get_ref()[0.._length]);
    //println!("read {:?}", packet);

    match packet {
        HazelMessage::Hello((_, hello)) => {
            client.id = hello.id;
            let msg = &state.config.message;
            let lang = u32::from(hello.language);
            let mid = (msg.unique_id * 5) + lang;
            let packet = if hello.id == mid {
                HazelMessage::Reliable((client.nonce, vec![Data::CacheAnnouncement]))
            } else {
                let mut msgsend = &msg.message[lang as usize];
                if msgsend.is_empty() {
                    msgsend = &msg.default_message;
                }
                HazelMessage::Reliable((client.nonce, vec![Data::Announcement((mid, msgsend.clone()))]))
            };
            //println!("write {:?}", packet);
            write_packet(packet, &mut buffer)?;
            state.socket.send_to(buffer.get_ref(), address)?;
        }
        HazelMessage::Disconnect => {
            state.client_deletion.push(address);
        }
        HazelMessage::Ack(_) => {
            //do i really wanna bother rn
        }
        _ => {}
    }
    //println!("{:02x?}", buffer.get_ref());
    Ok(())
}

fn main() {
    let config: config::Config = config::load_toml().expect("Failed to load configuration!");

    let mut state = Server {
        clients: HashMap::new(),
        client_deletion: Vec::new(),
        receive_buffer: Cursor::new(vec![0; 1024]),
        socket: UdpSocket::bind(
            SocketAddr::new(
                IpAddr::from_str(config.endpoint.ip.as_str()).unwrap_or(IpAddr::from_str("127.0.0.1").unwrap()),
                config.endpoint.port.unwrap_or(22024),
            )
        ).unwrap(),
        config: config.clone(),
    };

    println!("Listening on {}:{}", config.endpoint.ip, config.endpoint.port.unwrap_or(22024));

    loop {
        match state.socket.recv_from(state.receive_buffer.get_mut()) {
            Ok((size, address)) => {
                message_received(size, address, &mut state).unwrap_or(());//read with no errors! 😀
            }
            Err(err) => {
                eprintln!("{}", err)
            }
        }
    }
}