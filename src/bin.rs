use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::str::FromStr;

use announce_au::{Data, FreeWeekendState, HazelMessage, read_packet, write_packet};

use crate::config::Config;

pub(crate) mod config {
    use std::fs::{read_to_string, write};
    use std::io;

    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Config {
        pub endpoint: Endpoint,
        pub message: Message,
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Endpoint {
        pub ip: String,
        pub port: Option<u16>,
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Message {
        pub unique_id: u32,
        pub default_message: String,
        pub message: [String; 5],
    }

    pub fn create_toml() -> io::Result<Config> {
        let config = Config {
            endpoint: Endpoint {
                ip: "127.0.0.1".to_string(),
                port: Some(22023)
            },
            message: Message {
                unique_id: 0,
                default_message: "Change this message in config.toml and restart!".to_string(),
                message: ["".to_string(), "".to_string(), "".to_string(), "".to_string(), "".to_string()]
            }
        };
        write("./config.toml", toml::to_string_pretty(&config).unwrap().as_str())?;

        Ok(config)
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
                HazelMessage::Reliable((client.nonce, vec![Data::Announcement((mid, msgsend.clone())), Data::FreeWeekend(FreeWeekendState::Free)]))
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
    let config: config::Config = config::load_toml().unwrap_or_else(|_|config::create_toml().expect("Failed to create config"));

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
        if !state.client_deletion.is_empty() {
            for address in &state.client_deletion {
                state.clients.remove(&address);
            }
            state.client_deletion.clear();
        }
    }
}