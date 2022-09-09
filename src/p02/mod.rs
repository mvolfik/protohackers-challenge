use std::{
    io::{BufReader, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};

pub fn main() {
    let listener = TcpListener::bind("0.0.0.0:1200").unwrap();
    let lock: Arc<Mutex<()>> = Default::default();
    for incoming in listener.into_incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
        };
        let lock = Arc::clone(&lock);
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

                match op {
                    b'I' => {
                        prices.push((num1, num2));
                    }
                    b'Q' => {
                        prices.sort();
                        let start = prices.partition_point(|(timestamp, _)| !(*timestamp >= num1));
                        let end = prices.partition_point(|(timestamp, _)| *timestamp <= num2);
                        let n = end - start;
                        let sum = prices[start..end]
                            .iter()
                            .map(|(_, price)| price)
                            .sum::<i32>();
                        let mean = if n == 0 { 0 } else { sum / n as i32 };
                        let guard = lock.lock().unwrap();
                        eprintln!(
                            "Request: [{}, {}]; Response: {}; Start: {}; End: {}; N: {};",
                            num1, num2, mean, start, end, n
                        );
                        for (i, (ts, _)) in prices.iter().enumerate() {
                            eprintln!(
                                "{:6} {}{} {}",
                                i,
                                if *ts >= num1 { '#' } else { ' ' },
                                if *ts <= num2 { '#' } else { ' ' },
                                ts,
                            );
                        }
                        drop(guard);
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
