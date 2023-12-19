use std::io;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::thread;

fn main() -> io::Result<()> {
    let server_address = "127.0.0.1:34255";
    let mut stream = TcpStream::connect(server_address)?;

    let mut stream_clone = stream.try_clone()?;  // Create a clone for the main thread.

    // Spawn a thread to read messages from the server.
    thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    println!("Server disconnected");
                    break;
                }
                Ok(n) => {
                    let message = String::from_utf8_lossy(&buffer[0..n]);
                    println!("Received: {}", message);
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
        stream_clone.write_all(input.as_bytes())?;
        input.clear();
    }

    Ok(())
}