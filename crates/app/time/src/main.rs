#![no_std]
#![no_main]

use lib::*;

extern crate lib;

fn main() -> isize {
    let start = sys_time();
    let start_ms = sys_time_millis();

    println!("Current time: {}", start);
    println!("Sleeping for 1000 ms...");
    sleep(1000);

    let end = sys_time();
    let end_ms = sys_time_millis();

    println!("Current time: {}", end);
    println!("Elapsed: {} ms", end_ms - start_ms);

    0
}

entry!(main);
