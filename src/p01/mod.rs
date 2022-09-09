use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Request {
    method: String,
    number: u64,
}

#[derive(Serialize)]
struct Response {
    method: String,
    prime: bool,
}

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
                if read == 0 {
                    stream.shutdown(std::net::Shutdown::Both).unwrap();
                    break;
                }
                let request: Request = match serde_json::from_slice(&bytes[..bytes.len() - 1]) {
                    Ok(request) => request,
                    Err(e) => {
                        eprintln!("Error parsing request: {:?}", e);
                        send_malformed_and_close(&mut stream);
                        break;
                    }
                };

                match request.method.as_str() {
                    "isPrime" => {
                        stream
                            .write(
                                &serde_json::to_vec(&Response {
                                    method: "isPrime".to_string(),
                                    prime: is_prime(request.number),
                                })
                                .unwrap(),
                            )
                            .unwrap();
                    }
                    _ => {
                        eprintln!("Invalid method: {:?}", request.method);
                        send_malformed_and_close(&mut stream);
                        break;
                    }
                };
            }
        });
    }
}

fn send_malformed_and_close(stream: &mut TcpStream) {
    stream.write(&[b'{', b'\n']).unwrap();
    stream.shutdown(std::net::Shutdown::Both).unwrap();
}

fn is_prime(n: u64) -> bool {
    for i in 2..=(n as f64).sqrt() as u64 {
        if n % i == 0 {
            return false;
        }
    }
    true
}
