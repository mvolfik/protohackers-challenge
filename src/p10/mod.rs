use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};

enum Entry {
    File(Vec<Vec<u8>>),
    Directory(HashMap<String, Entry>),
}

const LEGAL_NONALPHANUM: &[char] = &['.', '_', '-', '/'];
fn is_name_illegal(n: &str, is_file: bool) -> bool {
    n.contains("//")
        || (is_file && n.ends_with('/'))
        || n.is_empty()
        || !n
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || LEGAL_NONALPHANUM.contains(&c))
}

pub fn main() {
    let listener = TcpListener::bind("0.0.0.0:1200").unwrap();
    let root = Arc::new(Mutex::new(HashMap::new()));
    let mut i = 0_u32;
    for incoming in listener.into_incoming() {
        i += 1;
        let i = i;
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
        };
        let root = Arc::clone(&root);
        std::thread::spawn(move || {
            stream.write_all(b"READY\n").unwrap();
            let EMPTY = HashMap::new();
            let mut buffer = BufReader::new(stream);
            let mut line = String::new();
            loop {
                line.clear();
                let read = buffer.read_line(&mut line).unwrap();
                if read == 0 {
                    buffer
                        .into_inner()
                        .shutdown(std::net::Shutdown::Both)
                        .unwrap();
                    break;
                }
                line.truncate(line.len() - 1);

                eprintln!("[{i}]Received request: {:?}", line);
                let words = line.split(' ').collect::<Vec<_>>();
                let mut reply = match (
                    words.get(0).map(|v| v.to_ascii_uppercase()).as_deref(),
                    words.len(),
                ) {
                    (None, _) => "ERR no command".to_owned(),
                    (Some("HELP"), _) => "OK you should know".to_owned(),
                    (Some("LIST"), 2) => {
                        if is_name_illegal(words[1], false) {
                            "ERR invalid directory".to_owned()
                        } else {
                            let guard = root.lock().unwrap();
                            let mut current = &*guard;
                            let mut parts = words[1].split('/');
                            loop {
                                match parts.next() {
                                    None => {
                                        let mut items = current.keys().collect::<Vec<_>>();
                                        items.sort();
                                        break std::iter::once(format!("OK {}", current.len()))
                                            .chain(items.into_iter().map(|k| match &current[k] {
                                                Entry::File(versions) => {
                                                    format!("{k} r{}", versions.len())
                                                }
                                                Entry::Directory(_) => {
                                                    format!("{k}/ DIR")
                                                }
                                            }))
                                            .intersperse_with(|| "\n".to_owned())
                                            .collect::<String>();
                                    }
                                    Some("") => {}
                                    Some(part) => match current.get(part) {
                                        Some(Entry::Directory(dir)) => {
                                            current = dir;
                                        }
                                        Some(Entry::File(_)) | None => {
                                            current = &EMPTY;
                                            while parts.next().is_some() {}
                                        }
                                    },
                                }
                            }
                        }
                    }
                    (Some("GET"), len @ 2) | (Some("GET"), len @ 3) => {
                        let name = words[1];
                        if is_name_illegal(name, true) {
                            "ERR invalid file".to_owned()
                        } else {
                            let guard = root.lock().unwrap();
                            let mut current = &*guard;
                            let mut parts = name.split('/').peekable();
                            let found = loop {
                                match parts.next() {
                                    None => break false,
                                    Some("") => {}
                                    Some(part) => match current.get(part) {
                                        Some(Entry::Directory(d)) => {
                                            current = d;
                                        }
                                        Some(Entry::File(versions)) => {
                                            if parts.peek().is_none() {
                                                let version = if len == 3 {
                                                    match words[2][1..].parse::<usize>() {
                                                        Ok(v) => v,
                                                        Err(_) => {
                                                            break false;
                                                        }
                                                    }
                                                } else {
                                                    versions.len()
                                                }
                                                .wrapping_sub(1);
                                                if version >= versions.len() {
                                                    break false;
                                                }
                                                buffer
                                                    .get_mut()
                                                    .write_all(
                                                        format!("OK {}\n", versions[version].len())
                                                            .as_bytes(),
                                                    )
                                                    .unwrap();
                                                buffer
                                                    .get_mut()
                                                    .write_all(&versions[version])
                                                    .unwrap();
                                                buffer.get_mut().write_all(b"READY\n").unwrap();
                                                break true;
                                            } else {
                                                break false;
                                            }
                                        }
                                        None => break false,
                                    },
                                }
                            };
                            if found {
                                continue;
                            } else {
                                "ERR file or version not found".to_owned()
                            }
                        }
                    }
                    (Some("PUT"), 3) => {
                        let name = words[1];
                        if is_name_illegal(name, true) {
                            "ERR invalid file".to_owned()
                        } else if let Ok(size) = words[2].parse::<usize>() {
                            let mut guard = root.lock().unwrap();
                            let mut current = &mut *guard;
                            let mut parts = name.split('/').peekable();
                            let revision = loop {
                                match parts.next() {
                                    None => break None,
                                    Some("") => {}
                                    Some(part) => {
                                        match current.entry(part.to_owned()).or_insert_with(|| {
                                            // FIXME: if reading fails after this, it will create a file with no revisions
                                            if parts.peek().is_none() {
                                                Entry::File(Vec::new())
                                            } else {
                                                Entry::Directory(HashMap::new())
                                            }
                                        }) {
                                            Entry::File(f) => {
                                                if parts.peek().is_none() {
                                                    let mut data = vec![0; size];
                                                    buffer.read_exact(&mut data).unwrap();
                                                    if f.last() != Some(&data) {
                                                        eprintln!(
                                                            "[{i}] data: {}",
                                                            String::from_utf8_lossy(&data)
                                                                .replace('\n', "[\\n]")
                                                        );
                                                        f.push(data);
                                                    } else {
                                                        eprintln!("[{i}] duplicate");
                                                    }
                                                    break Some(f.len());
                                                } else {
                                                    break None;
                                                }
                                            }
                                            Entry::Directory(d) => {
                                                current = d;
                                            }
                                        }
                                    }
                                }
                            };
                            if let Some(rev) = revision {
                                format!("OK r{}", rev)
                            } else {
                                "ERR invalid file".to_owned()
                            }
                        } else {
                            "ERR invalid size".to_owned()
                        }
                    }
                    (Some(x), n) => {
                        let mut writer = buffer.into_inner();
                        writer
                            .write_all(format!("ERR unknown command `{x}` or incorrect number of arguments ({n})\n").as_bytes())
                            .unwrap();
                        writer.shutdown(std::net::Shutdown::Both).unwrap();
                        break;
                    }
                };
                reply.push_str("\nREADY\n");
                buffer.get_mut().write_all(reply.as_bytes()).unwrap();
            }
        });
    }
}
