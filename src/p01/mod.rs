use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

use serde::{Deserialize, Serialize};
use serde_json::Number as JsonNumber;

#[derive(Deserialize, Debug)]
struct Request {
    method: String,
    number: JsonNumber,
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

                eprintln!("Received request: {:?}", request);
                match request.method.as_str() {
                    "isPrime" => {
                        let mut response = serde_json::to_vec(&Response {
                            method: "isPrime".to_string(),
                            prime: is_prime(request.number),
                        })
                        .unwrap();
                        response.push(b'\n');
                        stream.write_all(&response).unwrap();
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
    stream.write_all(&[b'{', b'\n']).unwrap();
    stream.shutdown(std::net::Shutdown::Both).unwrap();
}

fn is_prime(value: JsonNumber) -> bool {
    let Some(n) = value.as_u64() else { return false; };
    if n < 2 {
        return false;
    }
    for i in 2..=(n as f64).sqrt() as u64 {
        if n % i == 0 {
            return false;
        }
    }
    true
}
