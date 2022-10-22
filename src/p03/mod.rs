use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

pub fn main() {
    let listener = TcpListener::bind("0.0.0.0:1200").unwrap();
    let members: Arc<Mutex<Vec<(String, TcpStream)>>> = Default::default();
    for incoming in listener.into_incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
        };
        let members = Arc::clone(&members);
        std::thread::spawn(move || {
            let mut buffer = BufReader::new(stream.try_clone().unwrap());
            stream.write_all(b"Welcome. What's your name?\n").unwrap();
            let mut msg = String::new();
            buffer.read_line(&mut msg).unwrap();
            let name = msg.trim_end().to_owned();
            if name.len() < 1
                || !name.chars().all(|c| c.is_alphanumeric())
                || members
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(name2, _)| *name2 == name)
            {
                stream.write_all(b"Invalid name\n").unwrap();
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                return;
            }

            {
                let mut unlocked_members = members.lock().unwrap();
                let mut names = b"* Connected users:".to_vec();
                for (name, _) in &*unlocked_members {
                    names.push(b' ');
                    names.extend_from_slice(name.as_bytes());
                }
                names.push(b'\n');
                if let Err(e) = stream.write_all(&names) {
                    eprintln!("Error writing hello to stream {}: {:?}", name, e);
                    stream.shutdown(std::net::Shutdown::Both).unwrap();
                    return;
                }
                for (name2, stream) in &mut *unlocked_members {
                    let res: Result<(), std::io::Error> = try {
                        stream.write_all(b"* New chat member: ")?;
                        stream.write_all(name.as_bytes())?;
                        stream.write_all(b"\n")?;
                    };
                    if let Err(e) = res {
                        eprintln!("Error writing member intro to stream {}: {:?}", name2, e);
                        stream.shutdown(std::net::Shutdown::Both).unwrap();
                    }
                }
                unlocked_members.push((name.clone(), stream.try_clone().unwrap()));
            }
            loop {
                let mut msg = String::new();
                let res = buffer.read_line(&mut msg);
                if let Err(e) = res {
                    eprintln!(
                        "Error reading from stream {}: {:?}. Read buffer contents: {:?}",
                        name, e, msg
                    );
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    break;
                }
                if matches!(res, Ok(0)) {
                    eprintln!("Stream {} closed by peer", name);
                    break;
                }
                let msg = msg.trim_end().to_owned();
                {
                    let mut unlocked_members = members.lock().unwrap();
                    for (name2, stream) in unlocked_members.iter_mut() {
                        if name == *name2 {
                            continue;
                        }
                        let res: Result<(), std::io::Error> = try {
                            stream.write_all(b"[")?;
                            stream.write_all(name.as_bytes())?;
                            stream.write_all(b"] ")?;
                            stream.write_all(msg.as_bytes())?;
                            stream.write_all(b"\n")?;
                        };
                        if let Err(e) = res {
                            eprintln!("Error writing message to stream {}: {:?}", name2, e);
                            let _ = stream.shutdown(std::net::Shutdown::Both);
                        }
                    }
                }
            }
            {
                let mut unlocked_members = members.lock().unwrap();
                unlocked_members.retain(|(n, _)| n != &name);
                for (name2, stream) in &mut *unlocked_members {
                    let res: Result<(), std::io::Error> = try {
                        stream.write_all(b"* ")?;
                        stream.write_all(name.as_bytes())?;
                        stream.write_all(b" is no longer among us\n")?;
                    };
                    if let Err(e) = res {
                        eprintln!("Error writing close message to stream {}: {:?}", name2, e);
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    }
                }
            }
        });
    }
}
