use std::{
    io::{BufReader, Read, Write},
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
            let mut prices = Vec::<(i32, i32)>::new();
            let mut buffer = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut bytes = [0; 9];
                if let Err(e) = buffer.read_exact(&mut bytes) {
                    eprintln!(
                        "Error reading from stream: {:?}. Read buffer contents: {:?}",
                        e, bytes
                    );
                    stream.shutdown(std::net::Shutdown::Both).unwrap();
                    break;
                }
                let (op_b, rest) = bytes.split_at(1);
                let (num1_b, num2_b) = rest.split_at(4);
                let op = op_b[0];
                let num1 = i32::from_be_bytes(num1_b.try_into().unwrap());
                let num2 = i32::from_be_bytes(num2_b.try_into().unwrap());

                eprintln!("Received request: {} {} {}", op, num1, num2);
                match op {
                    b'I' => {
                        prices.push((num1, num2));
                    }
                    b'Q' => {
                        prices.sort();
                        let start = prices.partition_point(|(timestamp, _)| *timestamp >= num1);
                        let end = prices.partition_point(|(timestamp, _)| *timestamp < num2);
                        let n = end - start - 1;
                        let sum = prices[start..end]
                            .iter()
                            .map(|(_, price)| price)
                            .sum::<i32>();
                        let mean = if n == 0 { 0 } else { sum / n as i32 };
                        stream.write_all(&(mean).to_be_bytes()).unwrap();
                    }
                    _ => {
                        eprintln!("Invalid operation: {}", op);
                        stream.shutdown(std::net::Shutdown::Both).unwrap();
                        break;
                    }
                }
            }
        });
    }
}
