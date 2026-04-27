#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const MOD: u64 = 1_000_000_007;

fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1) % MOD
    }
}

fn main() -> isize {
    print!("Input n: ");

    let input = stdin().read_line();
    let input = input.trim();

    let n = match input.parse::<u64>() {
        Ok(n) => n,
        Err(_) => {
            println!("invalid input: {}", input);
            return 1;
        }
    };

    if n > 1_000_000 {
        println!("n must be less than 1000000");
        return 1;
    }

    let result = factorial(n);

    sys_stat();
    println!("The factorial of {} under modulo {} is {}.", n, MOD, result);

    0
}

entry!(main);
