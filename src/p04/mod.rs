pub fn main() {
    let socket = std::net::UdpSocket::bind("0.0.0.0:1200").unwrap();
    let mut storage = std::collections::HashMap::new();

    loop {
        let mut buf = [0; 1000];
        let (size, addr) = socket.recv_from(&mut buf).unwrap();
        let msg = std::str::from_utf8(&buf[..size]).unwrap();
        if msg == "version" {
            socket
                .send_to(b"version=Unusual Database Program v0.1", addr)
                .unwrap();
            continue;
        }
        if let Some(i) = msg.find('=') {
            let key = msg[..i].to_owned();
            let value = msg[i + 1..].to_owned();
            storage.insert(key, value);
            continue;
        }

        let value = storage.get(msg).map_or("", |v| v.as_str());
        socket
            .send_to(format!("{}={}", msg, value).as_bytes(), addr)
            .unwrap();
    }
}
