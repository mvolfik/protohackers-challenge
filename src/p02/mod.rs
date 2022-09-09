use std::{
    collections::LinkedList,
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
            let mut prices = LinkedList::<(i32, i32)>::new();
            let mut buffer = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut bytes = [0; 9];
                if let Err(e) = buffer.read_exact(&mut bytes) {
                    eprintln!("Error reading from stream: {:?}", e);
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
                        let mut cursor = prices.cursor_front_mut();
                        while cursor
                            .current()
                            .is_some_and(|(cursor_ts, _)| *cursor_ts < num1)
                        {
                            cursor.move_next();
                        }
                        cursor.insert_before((num1, num2));
                        eprintln!("Current list state: {:?}", prices);
                    }
                    b'Q' => {
                        let mut cursor = prices.cursor_front();
                        while cursor
                            .current()
                            .is_some_and(|(cursor_ts, _)| *cursor_ts < num1)
                        {
                            cursor.move_next();
                        }
                        let mut n = 0;
                        let mut sum = 0;
                        while let Some((cursor_ts, cursor_value)) = cursor.current() && *cursor_ts <= num2 {
                            n += 1;
                            sum += cursor_value;
                            cursor.move_next();
                        };
                        stream.write_all(&(sum / n).to_be_bytes()).unwrap();
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
