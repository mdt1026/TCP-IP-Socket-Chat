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
use std::net::{Shutdown, SocketAddr};
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
type Connections = HashMap<SocketAddr, TcpStream>;
lazy_static! {
    static ref SHARED_STREAMS: Arc<Mutex<HashMap<String, Chatroom>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref USERS: Arc<Mutex<HashMap<SocketAddr, String>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref CONNECTIONS: Arc<Mutex<Connections>> = Arc::new(Mutex::new(HashMap::new()));
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
                    CONNECTIONS.lock().unwrap().get(user).unwrap(),
                    format!("[Server]: {}", message) 
                ).unwrap();
            }
            println!("Done");
            Ok(())
        },
        None => Err("Chatroom does not exist")
    }
}

fn handle_broadcast(stream: &TcpStream, message: String) -> Result<(), &'static str> {
    let (_, chatroom) = find_user_chatroom(stream)?;
    let addr = stream.peer_addr().unwrap();
    for user in chatroom {
        if addr != user {
            send_message(
                CONNECTIONS.lock().unwrap().get(&user).unwrap(),
                format!("[{}]: {}", addr_to_username(&addr)?, message) 
            ).unwrap();
        }
    }
    Ok(())
}

fn handle_join(chatroom: String, stream: &TcpStream) -> Result<(), &'static str> {
    let mut s = SHARED_STREAMS.lock().unwrap();

    match s.get(&chatroom) {
        Some(&ref users) => {
            if users.contains(&stream.peer_addr().unwrap()) {
                return Err("User is already in chatroom")
            };
            let mut new_users = users.clone();
            new_users.push(stream.peer_addr().unwrap());
            *(s.get_mut(&chatroom).unwrap()) = new_users;
            let announcement_message = format!("{} has joined the chatroom.", addr_to_username(&stream.peer_addr().unwrap())?);
            std::mem::drop(s);
            handle_server_announcement(chatroom, announcement_message)?;
            return Ok(());
        },
        None => {
            s.insert(chatroom.clone(), vec![]);
            std::mem::drop(s);
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
    let chatroom = find_user_chatroom(&stream.try_clone().unwrap())?;
    let addr = &stream.peer_addr().unwrap();

    let mut users = chatroom.1;
    let index = users.iter().position(|n| n == addr).unwrap();
    users.remove(index);

    let mut s = SHARED_STREAMS.lock().unwrap();
    s.insert(chatroom.0.clone(), users);
    Ok(chatroom.0)
}

fn handle_leave(stream: &TcpStream) -> Result<(), &'static str> {
    let chatroom = remove_user_from_streams(stream)?;
    let announcement_message = format!("{} has left the chatroom.", addr_to_username(&stream.peer_addr().unwrap())?);
    handle_server_announcement(chatroom, announcement_message)?;
    Ok(())
}

fn handle_disconnect(stream: &TcpStream) -> Result<(), &'static str> {
    match remove_user_from_streams(stream) {
        Ok(_) => (),
        Err("User is not in a chatroom") => (),
        Err(e) => return Err(e)
    };
    let addr = &stream.peer_addr().unwrap();

    // Remove user from users list
    let u = &mut USERS.lock().unwrap();
    println!("{:?}", u);
    println!("{}", addr);
    u.remove(addr).unwrap();

    // Remove connection 
    let c = &mut CONNECTIONS.lock().unwrap();
    c.get_mut(addr).unwrap().shutdown(Shutdown::Both).unwrap();
    c.remove(addr).unwrap();

    Ok(())
}

fn handle_list(stream: &TcpStream) -> Result<(), &'static str> {
    let s = &SHARED_STREAMS.lock().unwrap();
    let message = s.keys().into_iter().map(|c| c.to_string()).collect::<Vec<String>>().join(", ");
    send_message(stream, message)?;
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
    let addr = &stream.peer_addr().unwrap();
    *(u.get_mut(addr).unwrap()) = nick; 
    Ok(())
}

fn handle_help(stream: &TcpStream) -> Result<(), &'static str> {
    let message = "Commands:\n\
                    \t\\help\n\
                    \t\\join <channel_name>\n\
                    \t\\disconnect\n\
                    \t\\list\n\
                    \t\\users\n\
                    \t\\leave\n\
                    \t\\nick <new_nick>";
    send_message(stream, message.to_string())?;
    Ok(())
}

fn parse_input(mut stream: &TcpStream) -> Result<(), &'static str> {
    let mut buffer = [0; 1024];

    println!("Shared Streams State: {:?}", SHARED_STREAMS.lock().unwrap());
    println!("Users State: {:?}", USERS.lock().unwrap());
    println!("Connections State: {:?}", CONNECTIONS.lock().unwrap());

    match stream.read(&mut buffer) {

        // Client has disconnected
        Ok(0) => panic!("Connection has disconnected"),

        Ok(n) => {
            let message = String::from_utf8_lossy(&buffer[0..n]);
            if message.chars().next().unwrap() == '/' {
                // Handle Commands
                let s = &message[1..message.len()-1];
                let (command, args): (&str, &str) = s.split_once(' ').unwrap_or((s, ""));
                let args_vec: Vec<&str>;
                if args == "" {
                    args_vec = Vec::new();
                } else {
                    args_vec = args.split(' ').collect();
                }
                match command {
                    "join" => {
                        if args_vec.len() != 1 {
                            return Err("Incorrect args passed to join command");
                        }
                        println!("Executing join");
                        Ok(handle_join(args_vec[0].to_string(), stream)?)
                    },
                    "disconnect" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the disconnect command");
                        }
                        Ok(handle_disconnect(stream)?)
                    },
                    "list" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the list command");
                        }
                        Ok(handle_list(stream)?)
                    },
                    "users" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the users command");
                        }
                        Ok(handle_users(stream)?)
                    },
                    "leave" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed to the leave command");
                        }
                        Ok(handle_leave(stream)?)
                    },
                    "nick" => {
                        if args_vec.len() != 1 { 
                            return Err("Incorrect args passed into the nick command");
                        }
                        Ok(handle_nick(stream, args_vec[0].to_string())?) 
                    }
                    "help" => {
                        if args_vec.len() != 0 {
                            return Err("Incorrect args passed into the help command");
                        }
                        Ok(handle_help(stream)?)
                    }
                    other => {
                        println!("Unknown command: {}", other);
                        Err("Unknown command")
                    }
                }
            } else {
                Ok(handle_broadcast(stream, message.to_string())?)
            }
        },
        Err(n) => {
            unimplemented!("Handle error: {n}")
        }
    }
}


fn send_server_message(stream: &TcpStream, mut message: String) -> Result<(), &'static str> {
    message = format!("[Server]: {}", message);
    Ok(send_message(stream, message)?)
}
fn send_message(mut stream: &TcpStream, message: String) -> Result<(), &'static str> {
    Ok(stream.write_all(message.as_bytes()).expect("Failed to write message"))
}

fn add_user_listener(ip: String) -> io::Result<()> {
    let listener = TcpListener::bind(ip)?;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                println!("Accepting a new connection");

                // Defualt username is a user hash. Add user to the USERS list
                let addr = stream.peer_addr().unwrap();
                let mut s = DefaultHasher::new();
                &addr.hash(&mut s);
                let uid = s.finish();
                USERS.lock().unwrap().insert(addr, format!("User{}", uid));
                
                // Add users to connections
                CONNECTIONS.lock().unwrap().insert(addr, stream.try_clone().unwrap());
                thread::spawn(move || {
                    loop {
                        match parse_input(&stream) {
                            Ok(_) => continue,
                            Err(e) => send_server_message(&stream, e.to_string()).unwrap(),
                        }
                    }
                });

                println!("Done");
            }
            Err(e) => {
                eprintln!("Error accepting a client: {}", e);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let ip = format!("{}:{}", args.ip, args.port);
    loop {
        add_user_listener(ip.clone()).unwrap();
    }
}
