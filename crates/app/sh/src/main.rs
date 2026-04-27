#![no_std]
#![no_main]

use lib::*;

extern crate lib;

fn print_help() {
    println!("YSOS Shell Help");
    println!("Student ID: 24319170");
    println!("help       - show this help message");
    println!("list       - list available user programs");
    println!("ps         - show process status");
    println!("run <app>  - spawn a user program and wait for it");
    println!("exit       - exit shell");
}

fn run_app(name: &str) {
    let pid = sys_spawn(name);
    if pid == 0 {
        println!("failed to spawn app: {}", name);
        return;
    }

    println!("spawned {} as pid {}", name, pid);
    let code = sys_wait_pid(pid);
    println!("process {} exited with code {}", pid, code);
}

fn main() -> isize {
    println!("Welcome to YSOS Shell");
    println!("Type `help` for usage.");

    loop {
        print!("sh(pid={})> ", sys_get_pid());
        let line = stdin().read_line();
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(cmd) = parts.next() else {
            continue;
        };

        match cmd {
            "help" => print_help(),
            "list" => sys_list_app(),
            "ps" | "stat" => sys_stat(),
            "run" => {
                if let Some(app) = parts.next() {
                    run_app(app);
                } else {
                    println!("usage: run <app>");
                }
            }
            "exit" => return 0,
            _ => println!("unknown command: {}", cmd),
        }
    }
}

entry!(main);
