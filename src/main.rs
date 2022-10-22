#![feature(let_chains)]
#![feature(linked_list_cursors)]
#![feature(tcplistener_into_incoming)]
#![feature(try_blocks)]
mod p00;
mod p01;
mod p02;
mod p03;

fn main() {
    let task: u8 = std::env::args().nth(1).unwrap().parse().unwrap();
    match task {
        0 => p00::main(),
        1 => p01::main(),
        2 => p02::main(),
        3 => p03::main(),
        _ => panic!("Invalid task number"),
    }
}
