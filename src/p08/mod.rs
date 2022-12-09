use std::{
    fmt::Debug,
    io::{BufRead, BufReader, Read, Write},
};

pub fn main() {
    let listener = std::net::TcpListener::bind("0.0.0.0:1200").unwrap();

    for incoming in listener.into_incoming() {
        let stream = match incoming {
            Err(e) => {
                eprintln!("Error accepting incoming stream: {:?}", e);
                continue;
            }
            Ok(x) => x,
        };
        std::thread::spawn(move || {
            let (reader, mut writer) = new_ISL(BufReader::new(stream.try_clone().unwrap()), stream);

            let mut buf_reader = BufReader::new(reader);
            loop {
                let mut line = String::new();
                buf_reader.read_line(&mut line).unwrap();
                let mut max = (0, String::new());
                for part in line.split(',') {
                    let (number, string) = part.split_once('x').unwrap();
                    let number: u32 = number.parse().unwrap();
                    if number > max.0 {
                        max = (number, string.to_string());
                    }
                }
                writer
                    .write(format!("{}x{}", max.0, max.1).as_bytes())
                    .unwrap();
            }
        });
    }
}

#[derive(Debug, Clone)]
enum CipherOperation {
    ReverseBits,
    XOR(u8),
    XORPos,
    Add(u8),
    AddPos,
}

struct InsecureSocketLayerReader<T: BufRead> {
    cipher: Vec<CipherOperation>,
    counter: u8,
    reader: T,
}

struct InsecureSocketLayerWriter<U: Write> {
    cipher: Vec<CipherOperation>,
    counter: u8,
    writer: U,
}

fn new_ISL<T: BufRead, U: Write>(
    mut reader: T,
    writer: U,
) -> (InsecureSocketLayerReader<T>, InsecureSocketLayerWriter<U>) {
    let mut buf = Vec::new();
    let mut cipher = Vec::new();
    loop {
        if buf.is_empty() {
            reader.read_until(0, &mut buf).unwrap();
            buf.reverse()
        }
        cipher.push(match buf.pop().unwrap() {
            0 => break,
            1 => CipherOperation::ReverseBits,
            2 => CipherOperation::XOR(buf.pop().unwrap()),
            3 => CipherOperation::XORPos,
            4 => CipherOperation::Add(buf.pop().unwrap()),
            5 => CipherOperation::AddPos,
            other => panic!("Invalid cipher definition: byte {}", other),
        })
    }
    let mut writer = InsecureSocketLayerWriter {
        cipher: cipher.clone(),
        counter: 0,
        writer,
    };
    'check: {
        for i in 0..u8::MAX {
            writer.counter = i;
            if (0..u8::MAX).any(|i| writer.encrypt_byte(i) != i) {
                break 'check;
            }
        }
        panic!("Cipher is no-op: {:?}", cipher);
    }
    return (
        InsecureSocketLayerReader {
            cipher,
            counter: 0,
            reader,
        },
        writer,
    );
}

impl<T: BufRead> InsecureSocketLayerReader<T> {
    fn decrypt_byte(&self, mut byte: u8) -> u8 {
        for operation in self.cipher.iter().rev() {
            match operation {
                CipherOperation::ReverseBits => byte = byte.reverse_bits(),
                CipherOperation::XOR(key) => byte ^= key,
                CipherOperation::XORPos => byte ^= self.counter,
                CipherOperation::Add(key) => byte = byte.wrapping_sub(*key),
                CipherOperation::AddPos => byte = byte.wrapping_sub(self.counter),
            }
        }
        return byte;
    }
}

impl<T: BufRead> Read for InsecureSocketLayerReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let number = self.reader.read(buf)?;
        eprintln!("--> {:?}", &buf[..number]);
        for i in 0..number {
            buf[i] = self.decrypt_byte(buf[i]);
            self.counter = self.counter.wrapping_add(1);
        }
        return Ok(number);
    }
}

impl<U: Write> InsecureSocketLayerWriter<U> {
    fn encrypt_byte(&self, mut byte: u8) -> u8 {
        for operation in &self.cipher {
            match operation {
                CipherOperation::ReverseBits => byte = byte.reverse_bits(),
                CipherOperation::XOR(key) => byte ^= key,
                CipherOperation::XORPos => byte ^= self.counter,
                CipherOperation::Add(key) => byte = byte.wrapping_add(*key),
                CipherOperation::AddPos => byte = byte.wrapping_add(self.counter),
            }
        }
        return byte;
    }
}

impl<U: Write> Write for InsecureSocketLayerWriter<U> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let encoded = buf
            .into_iter()
            .map(|byte| {
                let byte = self.encrypt_byte(*byte);
                self.counter = self.counter.wrapping_add(1);
                return byte;
            })
            .collect::<Vec<_>>();

        let res = self.writer.write(&encoded);
        let wrote = res.as_ref().copied().unwrap_or_default();
        eprintln!("<-- {:?}", &encoded[..wrote]);
        self.counter = self.counter.wrapping_sub((buf.len() - wrote) as u8);

        res
    }

    fn flush(&mut self) -> std::io::Result<()> {
        return self.writer.flush();
    }
}
