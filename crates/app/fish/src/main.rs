#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const FISH_ROUNDS: usize = 8;
const LT_PRINTS: usize = FISH_ROUNDS + (FISH_ROUNDS + 1) / 2;
const GT_PRINTS: usize = FISH_ROUNDS + FISH_ROUNDS / 2;
const UNDERSCORE_PRINTS: usize = FISH_ROUNDS;

static LT_SEM: Semaphore = Semaphore::new(0x400);
static GT_SEM: Semaphore = Semaphore::new(0x401);
static UNDERSCORE_SEM: Semaphore = Semaphore::new(0x402);
static ACK_SEM: Semaphore = Semaphore::new(0x403);

fn main() -> isize {
    init_sync();

    let lt_pid = spawn_fish_child(&LT_SEM, "<", LT_PRINTS);
    let gt_pid = spawn_fish_child(&GT_SEM, ">", GT_PRINTS);
    let underscore_pid = spawn_fish_child(&UNDERSCORE_SEM, "_", UNDERSCORE_PRINTS);

    println!("fish output:");
    emit_sequence();
    println!();

    sys_wait_pid(lt_pid);
    sys_wait_pid(gt_pid);
    sys_wait_pid(underscore_pid);

    println!("fish finished");
    remove_sync();

    0
}

fn init_sync() {
    LT_SEM.init(0);
    GT_SEM.init(0);
    UNDERSCORE_SEM.init(0);
    ACK_SEM.init(0);
}

fn remove_sync() {
    LT_SEM.remove();
    GT_SEM.remove();
    UNDERSCORE_SEM.remove();
    ACK_SEM.remove();
}

fn spawn_fish_child(sem: &'static Semaphore, token: &'static str, count: usize) -> u16 {
    let pid = sys_fork();
    if pid == 0 {
        fish_child(sem, token, count);
        sys_exit(0);
    }
    pid
}

fn fish_child(sem: &'static Semaphore, token: &'static str, count: usize) {
    for _ in 0..count {
        sem.wait();
        print!("{}", token);
        ACK_SEM.signal();
    }
}

fn emit_sequence() {
    for round in 0..FISH_ROUNDS {
        if round % 2 == 0 {
            emit_once(&LT_SEM);
            emit_once(&GT_SEM);
            emit_once(&LT_SEM);
        } else {
            emit_once(&GT_SEM);
            emit_once(&LT_SEM);
            emit_once(&GT_SEM);
        }
        emit_once(&UNDERSCORE_SEM);
    }
}

fn emit_once(sem: &Semaphore) {
    sem.signal();
    ACK_SEM.wait();
}

entry!(main);
