use std::{
    collections::HashMap,
    mem,
    net::{SocketAddr, UdpSocket},
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

const RETRANSMIT_TIMEOUT: Duration = Duration::from_secs(3);

pub fn main() {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:1200").unwrap());
    let mut sessions = HashMap::<u32, _>::new();
    let (tx, rx) = mpsc::channel();
    let rx_sock = Arc::clone(&socket);
    thread::spawn(move || {
        let mut buf = vec![0; 1024];
        loop {
            match rx_sock.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    let _: Option<()> = try {
                        if size == 0 {
                            None?;
                        }
                        let string = String::from_utf8(buf[..size].to_owned())
                            .ok()?
                            .replace("\\\\", "ň")
                            .replace("\\/", "č")
                            .replace('ň', "\\");
                        if !string.starts_with('/') || !string.ends_with('/') {
                            None?;
                        }
                        let mut parts = string
                            .split('/')
                            .skip(1)
                            .map(|p| p.replace("č", "/"))
                            .collect::<Vec<_>>();
                        assert_eq!(parts.pop(), Some("".to_string()));
                        eprintln!("{addr}: {parts:?}");
                        tx.send((parts, addr)).ok()?;
                    };
                }
                Err(e) => eprintln!("Error: {e:?}"),
            }
        }
    });
    let mut rx_dl = Instant::now() + RETRANSMIT_TIMEOUT;
    loop {
        let _: Option<()> = try {
            let (parts, addr) = match rx.recv_deadline(rx_dl) {
                Ok(size) => size,
                Err(_timeout) => {
                    for (id, (addr, _, _, tx_acked, tx_buf)) in &mut sessions {
                        send(&socket, *id, *addr, *tx_acked, tx_buf);
                    }
                    rx_dl = Instant::now() + RETRANSMIT_TIMEOUT;
                    continue;
                }
            };
            match (parts.get(0)?.as_ref(), parts.len()) {
                ("connect", 2) => {
                    let id = parts[1].parse().ok()?;
                    sessions.insert(id, (addr, 0, String::new(), 0, String::new()));
                    if let Err(e) = socket.send_to(format!("/ack/{id}/0/").as_bytes(), addr) {
                        eprintln!("Error: {e:?}");
                    }
                }
                ("data", 4) => {
                    let id = parts[1].parse().ok()?;
                    let (_, my_rx_pos, rx_buf, tx_acked, tx_buf) = sessions.get_mut(&id)?;
                    let start_pos: usize = parts[2].parse().ok()?;
                    if start_pos == *my_rx_pos {
                        rx_buf.push_str(&parts[3]);
                        *my_rx_pos += parts[3].len();
                        if let Err(e) =
                            socket.send_to(format!("/ack/{id}/{my_rx_pos}/").as_bytes(), addr)
                        {
                            eprintln!("Error: {e:?}");
                        }
                        eprintln!("{id}: {rx_buf:?}");
                        loop {
                            let Some(nl_pos) = rx_buf.find('\n') else { break; };
                            let mut line = rx_buf.split_off(nl_pos + 1);
                            mem::swap(&mut line, rx_buf);
                            assert_eq!(line.pop(), Some('\n'));
                            tx_buf.extend(line.chars().rev());
                            tx_buf.push('\n');
                        }
                        send(&socket, id, addr, *tx_acked, tx_buf);
                    }
                    if let Err(e) =
                        socket.send_to(format!("/ack/{id}/{my_rx_pos}/").as_bytes(), addr)
                    {
                        eprintln!("Error: {e:?}");
                    }
                }
                ("ack", 3) => {
                    let id = parts[1].parse().ok()?;
                    let (_, _, _, tx_acked, tx_buf) = sessions.get_mut(&id)?;
                    let acked: usize = parts[2].parse().ok()?;
                    if acked > *tx_acked {
                        *tx_acked = acked;
                        *tx_buf = tx_buf[acked - *tx_acked..].to_owned();
                        send(&socket, id, addr, *tx_acked, tx_buf);
                    }
                }
                ("close", 2) => {
                    let id = parts[1].parse().ok()?;
                    sessions.remove(&id);
                    socket
                        .send_to(format!("/close/{id}/").as_bytes(), addr)
                        .ok()?;
                }
                _ => {
                    eprintln!("Invalid message");
                }
            };
        };
    }
}

fn send(sock: &UdpSocket, id: u32, addr: SocketAddr, tx_acked: usize, tx_buf: &String) {
    if !tx_buf.is_empty() {
        if let Err(e) = sock.send_to(format!("/data/{id}/{tx_acked}/{tx_buf}/").as_bytes(), addr) {
            eprintln!("Error: {e:?}");
        }
    }
}
