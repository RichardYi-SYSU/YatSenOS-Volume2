#![no_std]
#![no_main]

use lib::{string::String, *};

extern crate lib;

fn print_help() {
    println!("YSOS Shell Help");
    println!("Student ID: 24319170");
    println!("help       - show this help message");
    println!("list       - list available user programs");
    println!("ls [path]  - list filesystem directory");
    println!("cat <path> - print a filesystem file");
    println!("ps         - show process status");
    println!("run <path> - spawn a user program from filesystem and wait for it");
    println!("exit       - exit shell");
}

fn cat_file(path: &str) {
    let Some(fd) = sys_open(path) else {
        println!("failed to open file: {}", path);
        return;
    };

    let mut buf = [0u8; 512];
    loop {
        let Some(count) = sys_read(fd, &mut buf) else {
            println!("failed to read file: {}", path);
            break;
        };

        if count == 0 {
            break;
        }

        print!("{}", String::from_utf8_lossy(&buf[..count]));
    }
    println!();

    if !sys_close(fd) {
        println!("failed to close file: {}", path);
    }
}

fn run_app(path: &str) {
    let pid = sys_spawn(path);
    if pid == 0 {
        println!("failed to spawn app: {}", path);
        return;
    }

    println!("spawned {} as pid {}", path, pid);
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
            "ls" => {
                let path = parts.next().unwrap_or("/");
                if !sys_list_dir(path) {
                    println!("failed to list directory: {}", path);
                }
            }
            "cat" => {
                if let Some(path) = parts.next() {
                    cat_file(path);
                } else {
                    println!("usage: cat <path>");
                }
            }
            "ps" | "stat" => sys_stat(),
            "run" => {
                if let Some(path) = parts.next() {
                    run_app(path);
                } else {
                    println!("usage: run <path>");
                }
            }
            "exit" => return 0,
            _ => println!("unknown command: {}", cmd),
        }
    }
}

entry!(main);
