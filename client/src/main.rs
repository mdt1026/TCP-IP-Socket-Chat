use std::{io, process};
use std::net::TcpStream;
use std::io::{Read, Write};
use std::thread;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ip: String,
    #[arg(short, long)]
    port: u32,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let server_address = format!("{}:{}", args.ip, args.port);
    let mut stream = TcpStream::connect(server_address)?;

    // Spawn a thread to read messages from the server.
    let mut read_stream = stream.try_clone().unwrap();
    thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match read_stream.read(&mut buffer) {
                Ok(0) => {
                    println!("Disconnected from server");
                    process::exit(0x0);
                }
                Ok(n) => {
                    let message = String::from_utf8_lossy(&buffer[0..n]);
                    println!("{}", message);
                }
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    // Main thread for sending messages to the server.
    let mut input = String::new();
    loop {
        io::stdin().read_line(&mut input)?;
        stream.write_all(input.as_bytes())?;
        input.clear();
   }
}
