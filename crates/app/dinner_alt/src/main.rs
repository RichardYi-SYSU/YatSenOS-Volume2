#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const PHILOSOPHER_COUNT: usize = 5;
const EAT_ROUNDS: usize = 3;

static CHOPSTICK: [Semaphore; PHILOSOPHER_COUNT] =
    semaphore_array![0x500, 0x501, 0x502, 0x503, 0x504];
static PRINT_LOCK: Semaphore = Semaphore::new(0x505);

fn main() -> isize {
    init_sync();

    let mut pids = [0u16; PHILOSOPHER_COUNT];
    for id in 0..PHILOSOPHER_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            ordered_philosopher(id);
            sys_exit(0);
        } else {
            pids[id] = pid;
        }
    }

    for pid in pids {
        sys_wait_pid(pid);
    }

    locked_print("ordered dinner finished");
    remove_sync();

    0
}

fn init_sync() {
    for chopstick in CHOPSTICK {
        chopstick.init(1);
    }
    PRINT_LOCK.init(1);
}

fn remove_sync() {
    for chopstick in CHOPSTICK {
        chopstick.remove();
    }
    PRINT_LOCK.remove();
}

fn ordered_philosopher(id: usize) {
    let left = id;
    let right = (id + 1) % PHILOSOPHER_COUNT;
    let first = min(left, right);
    let second = max(left, right);

    for round in 0..EAT_ROUNDS {
        locked_print_state(id, "thinking", round);
        sleep(jitter_ms(id, round, 11));

        CHOPSTICK[first].wait();
        locked_print_chopstick(id, "picked first", first, round);

        CHOPSTICK[second].wait();
        locked_print_chopstick(id, "picked second", second, round);

        locked_print_state(id, "eating", round);
        sleep(jitter_ms(id, round, 23));

        CHOPSTICK[second].signal();
        locked_print_chopstick(id, "released second", second, round);

        CHOPSTICK[first].signal();
        locked_print_chopstick(id, "released first", first, round);
    }

    locked_print_done(id);
}

fn min(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

fn jitter_ms(id: usize, round: usize, salt: i64) -> i64 {
    let now = sys_time_millis();
    let seed = now + (id as i64) * 19 + (round as i64) * 37 + salt;
    20 + seed.rem_euclid(30)
}

fn locked_print(message: &str) {
    PRINT_LOCK.wait();
    println!("{}", message);
    PRINT_LOCK.signal();
}

fn locked_print_state(id: usize, state: &str, round: usize) {
    PRINT_LOCK.wait();
    println!("ordered philosopher {} {} round {}", id, state, round);
    PRINT_LOCK.signal();
}

fn locked_print_chopstick(id: usize, action: &str, chopstick: usize, round: usize) {
    PRINT_LOCK.wait();
    println!(
        "ordered philosopher {} {} chopstick {} round {}",
        id, action, chopstick, round
    );
    PRINT_LOCK.signal();
}

fn locked_print_done(id: usize) {
    PRINT_LOCK.wait();
    println!("ordered philosopher {} finished", id);
    PRINT_LOCK.signal();
}

entry!(main);
