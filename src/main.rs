#![feature(deadline_api)]
#![feature(iter_intersperse)]
#![feature(let_chains)]
#![feature(linked_list_cursors)]
#![feature(new_uninit)]
#![feature(read_buf)]
#![feature(tcplistener_into_incoming)]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
mod p00;
mod p01;
mod p02;
mod p03;
mod p04;
mod p05;
mod p07;
mod p08;
mod p09;
mod p10;

fn main() {
    let task: u8 = std::env::args().nth(1).unwrap().parse().unwrap();
    match task {
        0 => p00::main(),
        1 => p01::main(),
        2 => p02::main(),
        3 => p03::main(),
        4 => p04::main(),
        5 => p05::main(),
        7 => p07::main(),
        8 => p08::main(),
        9 => p09::main(),
        10 => p10::main(),
        _ => panic!("Invalid task number"),
    }
}
