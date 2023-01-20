use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};

const LEGAL_NONALPHANUM: &[char] = &['.', '_', '-', '/'];
fn get_name_parts(mut n: &str, is_dir: bool) -> Option<impl Iterator<Item = &str>> {
    if !n.starts_with("/")
        || n.contains("//")
        || (!is_dir && n.ends_with('/'))
        || !n
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || LEGAL_NONALPHANUM.contains(&c))
    {
        return None;
    }
    if is_dir && n.ends_with('/') {
        n = &n[..n.len() - 1];
    }
    let mut iter = n.split('/');
    iter.next();
    Some(iter)
}

#[derive(Default)]
struct Entry(HashMap<String, Entry>, Vec<Vec<u8>>);

pub fn main() {
    let listener = TcpListener::bind("0.0.0.0:1200").unwrap();
    let root: Arc<Mutex<Entry>> = Default::default();
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

                eprintln!("[{i}] Received request: {:?}", line);
                let words = line.split(' ').collect::<Vec<_>>();
                let mut reply = match (
                    words.get(0).map(|v| v.to_ascii_uppercase()).as_deref(),
                    words.len(),
                ) {
                    (None, _) => "ERR no command".to_owned(),
                    (Some("HELP"), _) => "OK you should know".to_owned(),
                    (Some("LIST"), 2) => {
                        if let Some(mut parts) = get_name_parts(words[1], true) {
                            let guard = root.lock().unwrap();
                            let mut current_opt = Some(&*guard);
                            while let Some(current) = current_opt && let Some(next_name) = parts.next() {
                                current_opt = current.0.get(next_name);
                            }
                            if let Some(Entry(target_dir, _)) = current_opt {
                                let mut keys = target_dir.keys().collect::<Vec<_>>();
                                keys.sort();
                                std::iter::once(format!("OK {}", target_dir.len()))
                                    .chain(keys.into_iter().map(|k| {
                                        let item = &target_dir[k];
                                        if item.1.is_empty() {
                                            format!("{k}/ DIR")
                                        } else {
                                            format!("{k} r{}", item.1.len())
                                        }
                                    }))
                                    .intersperse_with(|| "\n".to_owned())
                                    .collect::<String>()
                            } else {
                                "ERR no such directory".to_owned()
                            }
                        } else {
                            "ERR invalid directory".to_owned()
                        }
                    }
                    (Some("GET"), len @ 2) | (Some("GET"), len @ 3) => {
                        let name = words[1];
                        if let Some(mut parts) = get_name_parts(name, false) {
                            let guard = root.lock().unwrap();
                            let mut current_opt = Some(&*guard);
                            while let Some(current) = current_opt && let Some(next_name) = parts.next() {
                                current_opt = current.0.get(next_name);
                            }
                            if let Some(Entry(_, versions)) = current_opt {
                                let version_res = if len == 3 {
                                    match words[2][1..].parse::<usize>() {
                                        Ok(v) => Ok(v),
                                        Err(_) => Err(()),
                                    }
                                } else {
                                    Ok(versions.len())
                                }
                                .map(|v| v.wrapping_sub(1));
                                if let Ok(v) = version_res && v < versions.len() {
                                    buffer
                                        .get_mut()
                                        .write_all(format!("OK {}\n", versions[v].len()).as_bytes())
                                        .unwrap();
                                    buffer.get_mut().write_all(&versions[v]).unwrap();
                                    buffer.get_mut().write_all(b"READY\n").unwrap();
                                    continue;
                                } else if version_res.is_ok() {
                                    format!("ERR no such version {}", words[2])
                                } else {
                                    format!("ERR invalid version {}", words[2])
                                }
                            } else {
                                "ERR no such file".to_owned()
                            }
                        } else {
                            "ERR invalid file".to_owned()
                        }
                    }
                    (Some("PUT"), 3) => {
                        let name = words[1];
                        if let Some(parts) = get_name_parts(name, false) {
                            if let Ok(size) = words[2].parse::<usize>() {
                                let mut guard = root.lock().unwrap();
                                let mut current = &mut *guard;
                                for part in parts {
                                    current = current.0.entry(part.to_owned()).or_default()
                                }
                                let mut data = vec![0; size];
                                buffer.read_exact(&mut data).unwrap();
                                if !data.iter().all(|b| b.is_ascii_graphic() || b.is_ascii_whitespace()) {
                                    "ERR invalid data".to_owned()
                                } else {
                                    if current.1.last() == Some(&data) {
                                        eprintln!("[{i:0>3}] duplicate");
                                    } else {
                                        eprintln!(
                                            "[{i}] data: {}",
                                            String::from_utf8_lossy(&data).replace('\n', "[\\n]")
                                        );
                                        current.1.push(data);
                                    }
                                    format!("OK r{}", current.1.len())
                                }
                            } else {
                                "ERR invalid size".to_owned()
                            }
                        } else {
                            "ERR invalid file".to_owned()
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
