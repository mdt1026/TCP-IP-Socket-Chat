use std::io;
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::thread;
use std::sync::{Arc, Mutex};
use std::io::Read;
use std::io::Write;
use clap::Parser;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ip: String,
    #[arg(short, long)]
    port: u32,
}

fn handle_broadcast(message: String) -> () {
    
}

fn parse_input(stream: TcpStream) -> () {
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
    let shared_streams: Arc<Mutex<HashMap<String, Vec<TcpStream>>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let shared_streams_clone = shared_streams.clone();

                // Add the new client's stream to the shared_streams list.
                {
                    let mut shared_streams = shared_streams_clone.lock().unwrap();
                    shared_streams.push(stream.try_clone().expect("Failed to clone the stream"));
                }

                // Spawn a new thread to handle the client.
                thread::spawn(move || {
                    handle_client(&mut stream, shared_streams_clone);
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
