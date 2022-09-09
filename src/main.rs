#![feature(let_else)]
#![feature(tcplistener_into_incoming)]
mod p00;
mod p01;

fn main() {
    let task: u8 = std::env::args().nth(1).unwrap().parse().unwrap();
    match task {
        0 => p00::main(),
        1 => p01::main(),
        _ => panic!("Invalid task number"),
    }
}
