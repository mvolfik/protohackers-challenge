use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

pub fn main() {
    let listener = std::net::TcpListener::bind("0.0.0.0:1200").unwrap();

    for (i, incoming) in listener.into_incoming().enumerate() {
        let stream = match incoming {
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
            Ok(x) => x,
        };
        let buffer = BufReader::new(stream.try_clone().unwrap());
        let upstream = std::net::TcpStream::connect("206.189.113.124:16963").unwrap();
        let upstream_buffer = BufReader::new(upstream.try_clone().unwrap());
        std::thread::spawn(move || {
            proxy(buffer, upstream, format!("[{}] client -> server", i));
        });
        std::thread::spawn(move || {
            proxy(upstream_buffer, stream, format!("[{}] server -> client", i));
        });
    }
}

fn proxy(mut source: BufReader<TcpStream>, mut dest: TcpStream, hint: String) {
    loop {
        let mut msg = String::new();
        let res = source.read_line(&mut msg);
        match res {
            Ok(0) => {
                eprintln!("End of stream {}", hint);
                break;
            }
            Err(e) => {
                eprintln!("Error reading from stream {}: {:?}", hint, e);
                break;
            }
            _ => {}
        }
        let mut output = String::new();
        for part in msg.split(' ') {
            if part.chars().nth(0) == Some('7')
                && part.len() >= 26
                && part.len() <= 35
                && part.chars().all(|c| c.is_alphanumeric())
            {
                output += "7YWHMfk9JZe0LM0g1ZauHuiSxhI";
            } else {
                output += part;
            }
            output += " ";
        }
        output.pop();
        eprintln!("S: {:?}\nT: {:?}", msg, output);
        if let Err(e) = dest.write_all(output.as_bytes()) {
            eprintln!("Error writing to stream {}: {:?}", hint, e);
            break;
        }
    }
    source.get_ref().shutdown(std::net::Shutdown::Both);
    dest.shutdown(std::net::Shutdown::Both);
}
