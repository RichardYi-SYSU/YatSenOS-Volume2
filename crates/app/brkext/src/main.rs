#![no_std]
#![no_main]

use lib::{vec::Vec, *};

extern crate lib;

const TEST_SIZE: usize = 32 * 1024;

fn main() -> isize {
    let before = sys_brk(None).expect("failed to query heap end before allocation");
    println!("heap end before allocation: {:#x}", before);

    let mut data = Vec::with_capacity(TEST_SIZE);
    for i in 0..TEST_SIZE {
        data.push((i % 251) as u8);
    }

    for i in 0..TEST_SIZE {
        assert_eq!(data[i], (i % 251) as u8);
    }

    let after = sys_brk(None).expect("failed to query heap end after allocation");
    println!("heap end after allocation : {:#x}", after);
    println!("allocated {} bytes and verified contents", TEST_SIZE);
    assert!(after > before);

    println!("brk allocator extend test passed.");

    0
}

entry!(main);
