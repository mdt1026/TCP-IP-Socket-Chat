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

fn addr_to_username(addr: &SocketAddr) -> Result<String, &'static str> {
    let u = USERS.lock().unwrap();
    Ok(u.get(addr).unwrap().to_string())
}

fn handle_server_announcement(chatroom: String, message: String) -> Result<(), &'static str> {
    let s = SHARED_STREAMS.lock().unwrap();
    match s.get(&chatroom) {
        Some(&ref users) => {
            for user in users {
                send_message(
                    &TcpStream::connect(user).unwrap(),
                    format!("[Server]: {}", message) 
                ).unwrap();
            }
            Ok(())
        },
        None => Err("Chatroom does not exist")
    }
}

fn handle_broadcast(stream: &TcpStream, message: String) -> Result<(), &'static str> {
    let (_, chatroom) = find_user_chatroom(stream).unwrap();
    let addr = stream.peer_addr().unwrap();
    for user in chatroom {
        if addr != user {
            send_message(
                &TcpStream::connect(user).unwrap(),
                format!("[{}]: {}", addr_to_username(&addr).unwrap(), message) 
            ).unwrap();
        }
    }
    Ok(())
}

fn handle_join(chatroom: String, stream: &TcpStream) -> Result<(), &'static str> {
    let s = &mut SHARED_STREAMS.lock().unwrap();
    match s.get(&chatroom) {
        Some(&ref users) => {
            if users.contains(&stream.peer_addr().unwrap()) {
                return Err("User already exists")
            };
            let mut new_users = users.clone();
            new_users.push(stream.peer_addr().unwrap());
            s.get(&chatroom).replace(&new_users);
            handle_server_announcement(chatroom, 
                                       format!(
                                           "{} has joined the chatroom.",
                                           addr_to_username(&stream.peer_addr().unwrap()).unwrap()
                                               )
                                       ).unwrap();
            Ok(())
        },
        None => {
            s.insert(chatroom.clone(), vec![]);
            handle_join(chatroom, stream)
        },
    }
}

fn find_user_chatroom(stream: &TcpStream) -> Result<(String, Chatroom), &'static str> {
    let s = &SHARED_STREAMS.lock().unwrap();
    let addr = stream.peer_addr().unwrap();
    for (chat_name, chatroom) in s.iter() {
        if chatroom.contains(&addr) {
            return Ok((chat_name.to_owned(), chatroom.to_owned()));
        }
    }
    return Err("User is not in a chatroom");
}

/**
 * Returns the chatroom name that the user left from
 */
fn remove_user_from_streams(stream: &TcpStream) -> Result<String, &'static str> {
    let s = &mut SHARED_STREAMS.lock().unwrap();
    let chatroom = find_user_chatroom(&stream.try_clone().unwrap()).unwrap();
    let addr = &stream.peer_addr().unwrap();

    let mut users = chatroom.1;
    let index = users.iter().position(|n| n == addr).unwrap();
    users.remove(index);

    s.insert(chatroom.0.clone(), users);
    Ok(chatroom.0)
}

fn handle_leave(stream: &TcpStream) -> Result<(), &'static str> {
    let chatroom = remove_user_from_streams(stream).unwrap();
    handle_server_announcement(chatroom, 
                                       format!(
                                           "{} has joined the chatroom.",
                                           addr_to_username(&stream.peer_addr().unwrap()).unwrap()
                                               )
                                       ).unwrap();
    Ok(())
}

fn handle_disconnect(stream: &TcpStream) -> Result<(), &'static str> {
    remove_user_from_streams(stream).unwrap();

    // Remove user from users list
    let u = &mut USERS.lock().unwrap();
    u.remove(&stream.peer_addr().unwrap());

    Ok(())
}

fn handle_list(stream: &TcpStream) -> Result<(), &'static str> {
    let s = &SHARED_STREAMS.lock().unwrap();
    let message = s.keys().into_iter().map(|c| c.to_string()).collect::<Vec<String>>().join(", ");
    send_message(stream, message).unwrap();
    Ok(())
}

fn handle_users(stream: &TcpStream) -> Result<(), &'static str> {
    let mut res = vec![];

    // Generate the users list
    let s = &SHARED_STREAMS.lock().unwrap();
    for (chat_name, chatroom) in s.iter() {
        res.push(format!("{}: {}", chat_name, chatroom.into_iter().map(|c| addr_to_username(c).unwrap()).collect::<Vec<String>>().join(", "))); 
    };
    send_message(stream, res.into_iter().collect::<Vec<String>>().join("\n")).unwrap();
    Ok(())
}

fn handle_nick(stream: &TcpStream, nick: String) -> Result<(), &'static str> {
    let u = &mut USERS.lock().unwrap();
    u.get(&stream.peer_addr().unwrap()).replace(&nick);
    Ok(())
}

fn parse_input(mut stream: &TcpStream) -> Result<(), &'static str> {
    let mut buffer = [0, 255];
    match &stream.read(&mut buffer) {

        // Client has disconnected
        Ok(0) => Ok(handle_disconnect(stream).unwrap()),

        Ok(n) => {
            let message = String::from_utf8_lossy(&buffer[0..*n]);
            println!("{:?}", message);
            if message.chars().next().unwrap() == '/' {
                // Handle Commands
                let s = &message[1..];
                let (command, args): (&str, &str) = s.rsplit_once('|').unwrap();
                let args_vec: Vec<&str> = args.split(' ').collect();
                match command {
                    "join" => {
                        if args_vec.len() != 1 {
                            return Err("Incorrect args passed to join command");
                        }
                        Ok(handle_join(args_vec[0].to_string(), stream).unwrap())
                    },
                    "disconnect" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the disconnect command");
                        }
                        Ok(handle_disconnect(stream).unwrap())
                    },
                    "list" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the list command");
                        }
                        Ok(handle_list(stream).unwrap())
                    },
                    "users" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the users command");
                        }
                        Ok(handle_users(stream).unwrap())
                    },
                    "leave" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the users command");
                        }
                        Ok(handle_leave(stream).unwrap())
                    },
                    "nick" => {
                        if args_vec.len() != 1 { 
                            return Err("Incorrect args passed into the nick command");
                        }
                        Ok(handle_nick(stream, args_vec[0].to_string()).unwrap()) 
                    }
                    other => Err("Unknown command")
                }
            } else {
                Ok(handle_broadcast(stream, message.to_string()).unwrap())
            }
        },
        Err(n) => {
            unimplemented!("Handle error: {n}")
        }
    }
}

fn send_message(mut stream: &TcpStream, message: String) -> Result<(), &'static str> {
    Ok(stream.write_all(message.as_bytes()).expect("Failed to write message"))
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let ip = format!("{}:{}", args.ip, args.port);

    let listener = TcpListener::bind(ip)?;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {

                // Defualt username is a user hash. Add user to the USERS list
                let addr = stream.peer_addr().unwrap();
                let mut s = DefaultHasher::new();
                &addr.hash(&mut s);
                let uid = s.finish();
                USERS.lock().unwrap().insert(addr, format!("User{}", uid));

                // Spawn a new thread to handle the client.
                thread::spawn(move || {
                    match parse_input(&stream) {
                        Ok(_) => (),
                        Err(e) => send_message(&stream, e.to_string()).unwrap(),
                    };
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
