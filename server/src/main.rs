use std::io;
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::thread;
use std::sync::{Arc, Mutex};
use std::io::Read;
use std::io::Write;
use clap::Parser;
use std::collections::HashMap;
use lazy_static::lazy_static;
use std::net::SocketAddr;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ip: String,
    #[arg(short, long)]
    port: u32,
}

type Chatroom = Vec<SocketAddr>;
lazy_static! {
    static ref SHARED_STREAMS: Arc<Mutex<HashMap<String, Chatroom>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref USERS: Arc<Mutex<HashMap<SocketAddr, String>>> = Arc::new(Mutex::new(HashMap::new()));
}

fn handle_broadcast(message: String) -> () {
 
}

fn handle_join(chatroom: String, stream: TcpStream) -> Result<(), &'static str> {
    let s = &mut SHARED_STREAMS.lock().unwrap();
    match s.get(&chatroom) {
        Some(&ref users) => {
            if users.contains(&stream.peer_addr().unwrap()) {
                return Err("User already exists")
            };
            let mut new_users = users.clone();
            new_users.push(stream.peer_addr().unwrap());
            s.get(&chatroom).replace(&new_users);
            Ok(())
        },
        None => {
            s.insert(chatroom.clone(), vec![]);
            handle_join(chatroom, stream)
        },
    }
}

fn find_user_chatroom(stream: TcpStream) -> Result<(String, Chatroom), &'static str> {
    let s = &SHARED_STREAMS.lock().unwrap();
    let addr = stream.peer_addr().unwrap();
    for (chat_name, chatroom) in s.iter() {
        if chatroom.contains(&addr) {
            return Ok((chat_name.to_owned(), chatroom.to_owned()));
        }
    }
    return Err("User is not in a chatroom");
}

fn handle_disconnect(stream: TcpStream) -> Result<(), &'static str> {
    // Remove user from shared streams
    let s = &mut SHARED_STREAMS.lock().unwrap();
    let chatroom = find_user_chatroom(stream.try_clone().unwrap()).unwrap();
    let addr = &stream.peer_addr().unwrap();

    let users = chatroom.1;

    let index = users.iter().position(|n| n == addr).unwrap();
    s.insert(chatroom.0, users);

    // Remove user from users list
    let u = &mut USERS.lock().unwrap();
    u.remove(addr);

    Ok(())
}

fn parse_input(mut stream: TcpStream) -> () {
    let mut buffer = [0, 1024];
    match &stream.read(&mut buffer) {
        Ok(0) => unimplemented!("TODO client disconnected"),
        Ok(n) => {
            let message = String::from_utf8_lossy(&buffer[0..*n]);
            if message.chars().next().unwrap() == '/' {
                // Handle Commands
                let s = &message[1..];
                let (command, args): (&str, &str) = s.rsplit_once('|').unwrap();
                let args_vec: Vec<&str> = args.split(' ').collect();
                match command {
                    "join" => {
                        // Add stream to handler function call
                        // handle_join(args_vec[0].to_string(), ;
                        assert!(args_vec.len() > 0);
                        unimplemented!("join command handler")
                    },
                    "disconnect" => unimplemented!("disconnect command handler"),
                    "list" => unimplemented!("list command handler"),
                    "users" => unimplemented!("users command handler"),
                    "leave" => unimplemented!("leavel command handler")
                }
            } else {
                unimplemented!("broadcast message")
            }
        },
        Err(n) => {
            unimplemented!("Handle errors")
        }
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let ip = format!("{}:{}", args.ip, args.port);

    let listener = TcpListener::bind(ip)?;

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let addr = stream.peer_addr().unwrap();
                let mut s = DefaultHasher::new();
                &addr.hash(&mut s);
                let uid = s.finish();
                USERS.lock().unwrap().insert(addr, format!("User{}", uid));

                // Spawn a new thread to handle the client.
                thread::spawn(move || {
                    parse_input(stream);
                });
            }
            Err(e) => {
                eprintln!("Error accepting a client: {}", e);
            }
        }
    }
}

fn handle_client(stream: &mut TcpStream, shared_streams: Arc<Mutex<Vec<TcpStream>>>) {
    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Client disconnected
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[0..n]);

                // Broadcast the message to all other clients
                let shared_streams = shared_streams.lock().unwrap();
                for mut client_stream in shared_streams.iter() {
                    if client_stream.as_raw_fd() != stream.as_raw_fd() {
                        client_stream.write_all(message.as_bytes()).expect("Failed to write message");
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from client: {}", e);
                break;
            }
        }
    }
}
