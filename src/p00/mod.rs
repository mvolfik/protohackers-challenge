use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

pub fn main() {
    let listener = TcpListener::bind("0.0.0.0:1200").unwrap();
    for incoming in listener.into_incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
        };
        std::thread::spawn(move || {
            let mut buffer = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut bytes = Vec::new();
                let read = buffer.read_until(b'\n', &mut bytes).unwrap();
                eprintln!("Read {} bytes: {:?}", read, bytes);
                if read == 0 {
                    break;
                }
                stream.write(&bytes).unwrap();
            }
        });
    }
}
