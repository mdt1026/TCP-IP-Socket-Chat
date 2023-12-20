use std::io;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::thread;

fn main() -> io::Result<()> {
    let server_address = "127.0.0.1:34257";
    let mut stream = TcpStream::connect(server_address)?;

    // Spawn a thread to read messages from the server.
    let mut read_stream = stream.try_clone().unwrap();
    thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match read_stream.read(&mut buffer) {
                Ok(0) => {
                    println!("Server disconnected");
                    break;
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
