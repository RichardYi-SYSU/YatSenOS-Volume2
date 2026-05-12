#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const PHILOSOPHER_COUNT: usize = 5;
const SAFE_ROUNDS: usize = 3;
const STARVE_ROUNDS: usize = 3;

static SAFE_CHOPSTICK: [Semaphore; PHILOSOPHER_COUNT] =
    semaphore_array![0x300, 0x301, 0x302, 0x303, 0x304];
static SAFE_WAITER: Semaphore = Semaphore::new(0x305);
static SAFE_PRINT_LOCK: Semaphore = Semaphore::new(0x306);

static STARVE_CHOPSTICK: [Semaphore; PHILOSOPHER_COUNT] =
    semaphore_array![0x310, 0x311, 0x312, 0x313, 0x314];
static STARVE_PRINT_LOCK: Semaphore = Semaphore::new(0x315);

static DEADLOCK_CHOPSTICK: [Semaphore; PHILOSOPHER_COUNT] =
    semaphore_array![0x320, 0x321, 0x322, 0x323, 0x324];
static DEADLOCK_PRINT_LOCK: Semaphore = Semaphore::new(0x325);

fn main() -> isize {
    run_safe_dinner();
    run_starvation_attempt();
    run_deadlock_attempt();

    0
}

fn run_safe_dinner() {
    init_safe_sync();
    locked_print(&SAFE_PRINT_LOCK, "=== safe dinner with waiter ===");

    let mut pids = [0u16; PHILOSOPHER_COUNT];
    for id in 0..PHILOSOPHER_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            safe_philosopher(id);
            sys_exit(0);
        } else {
            pids[id] = pid;
        }
    }

    for pid in pids {
        sys_wait_pid(pid);
    }

    locked_print(&SAFE_PRINT_LOCK, "safe dinner finished");
    remove_safe_sync();
}

fn init_safe_sync() {
    for chopstick in SAFE_CHOPSTICK {
        chopstick.init(1);
    }
    SAFE_WAITER.init(PHILOSOPHER_COUNT - 1);
    SAFE_PRINT_LOCK.init(1);
}

fn remove_safe_sync() {
    for chopstick in SAFE_CHOPSTICK {
        chopstick.remove();
    }
    SAFE_WAITER.remove();
    SAFE_PRINT_LOCK.remove();
}

fn safe_philosopher(id: usize) {
    let left = id;
    let right = (id + 1) % PHILOSOPHER_COUNT;

    for round in 0..SAFE_ROUNDS {
        locked_print_state(&SAFE_PRINT_LOCK, "safe", id, "thinking", round);
        sleep(jitter_ms(id, round, 13));

        SAFE_WAITER.wait();
        locked_print_state(&SAFE_PRINT_LOCK, "safe", id, "asks waiter", round);

        SAFE_CHOPSTICK[left].wait();
        locked_print_chopstick(&SAFE_PRINT_LOCK, "safe", id, "picked left", left, round);

        SAFE_CHOPSTICK[right].wait();
        locked_print_chopstick(&SAFE_PRINT_LOCK, "safe", id, "picked right", right, round);

        locked_print_state(&SAFE_PRINT_LOCK, "safe", id, "eating", round);
        sleep(jitter_ms(id, round, 29));

        SAFE_CHOPSTICK[right].signal();
        locked_print_chopstick(&SAFE_PRINT_LOCK, "safe", id, "released right", right, round);

        SAFE_CHOPSTICK[left].signal();
        locked_print_chopstick(&SAFE_PRINT_LOCK, "safe", id, "released left", left, round);

        SAFE_WAITER.signal();
    }

    locked_print_done(&SAFE_PRINT_LOCK, "safe", id);
}

fn run_starvation_attempt() {
    init_starve_sync();
    locked_print(&STARVE_PRINT_LOCK, "=== starvation attempt ===");

    let mut pids = [0u16; PHILOSOPHER_COUNT];
    for id in 0..PHILOSOPHER_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            starve_philosopher(id);
            sys_exit(0);
        } else {
            pids[id] = pid;
        }
    }

    sleep(120);
    locked_print(
        &STARVE_PRINT_LOCK,
        "starvation checkpoint: philosopher 0 has requested chopstick 0 but has not eaten yet",
    );
    sys_stat();

    for pid in pids {
        sys_wait_pid(pid);
    }

    locked_print(&STARVE_PRINT_LOCK, "starvation attempt finished");
    remove_starve_sync();
}

fn init_starve_sync() {
    for chopstick in STARVE_CHOPSTICK {
        chopstick.init(1);
    }
    STARVE_PRINT_LOCK.init(1);
}

fn remove_starve_sync() {
    for chopstick in STARVE_CHOPSTICK {
        chopstick.remove();
    }
    STARVE_PRINT_LOCK.remove();
}

fn starve_philosopher(id: usize) {
    match id {
        0 => starved_philosopher(),
        4 => neighbor_holding_needed_chopstick(),
        _ => short_guest(id),
    }
}

fn starved_philosopher() {
    sleep(20);
    locked_print(
        &STARVE_PRINT_LOCK,
        "starvation philosopher 0 requests chopstick 0",
    );
    STARVE_CHOPSTICK[0].wait();
    locked_print_chopstick(&STARVE_PRINT_LOCK, "starvation", 0, "picked left", 0, 0);

    STARVE_CHOPSTICK[1].wait();
    locked_print_chopstick(&STARVE_PRINT_LOCK, "starvation", 0, "picked right", 1, 0);

    locked_print_state(&STARVE_PRINT_LOCK, "starvation", 0, "eating", 0);
    sleep(40);

    STARVE_CHOPSTICK[1].signal();
    STARVE_CHOPSTICK[0].signal();
    locked_print_done(&STARVE_PRINT_LOCK, "starvation", 0);
}

fn neighbor_holding_needed_chopstick() {
    STARVE_CHOPSTICK[4].wait();
    STARVE_CHOPSTICK[0].wait();

    for round in 0..STARVE_ROUNDS {
        locked_print_state(&STARVE_PRINT_LOCK, "starvation", 4, "eating", round);
        sleep(80);
    }

    STARVE_CHOPSTICK[0].signal();
    STARVE_CHOPSTICK[4].signal();
    locked_print_done(&STARVE_PRINT_LOCK, "starvation", 4);
}

fn short_guest(id: usize) {
    let left = id;
    let right = (id + 1) % PHILOSOPHER_COUNT;

    STARVE_CHOPSTICK[left].wait();
    STARVE_CHOPSTICK[right].wait();
    locked_print_state(&STARVE_PRINT_LOCK, "starvation", id, "eating", 0);
    sleep(30);
    STARVE_CHOPSTICK[right].signal();
    STARVE_CHOPSTICK[left].signal();
    locked_print_done(&STARVE_PRINT_LOCK, "starvation", id);
}

fn run_deadlock_attempt() {
    init_deadlock_sync();
    locked_print(&DEADLOCK_PRINT_LOCK, "=== deadlock attempt ===");

    for id in 0..PHILOSOPHER_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            deadlocking_philosopher(id);
            sys_exit(0);
        }
    }

    sleep(3000);
    locked_print(
        &DEADLOCK_PRINT_LOCK,
        "deadlock checkpoint: all philosophers hold left chopstick and wait for right",
    );
    sys_stat();
    locked_print(
        &DEADLOCK_PRINT_LOCK,
        "deadlock attempt recorded; parent exits without waiting for blocked children",
    );
}

fn init_deadlock_sync() {
    for chopstick in DEADLOCK_CHOPSTICK {
        chopstick.init(1);
    }
    DEADLOCK_PRINT_LOCK.init(1);
}

fn deadlocking_philosopher(id: usize) {
    let left = id;
    let right = (id + 1) % PHILOSOPHER_COUNT;

    DEADLOCK_CHOPSTICK[left].wait();
    locked_print_chopstick(&DEADLOCK_PRINT_LOCK, "deadlock", id, "picked left", left, 0);

    sleep(100);
    locked_print_chopstick(
        &DEADLOCK_PRINT_LOCK,
        "deadlock",
        id,
        "waits right",
        right,
        0,
    );
    DEADLOCK_CHOPSTICK[right].wait();

    locked_print_state(&DEADLOCK_PRINT_LOCK, "deadlock", id, "unexpected eating", 0);
}

fn jitter_ms(id: usize, round: usize, salt: i64) -> i64 {
    let now = sys_time_millis();
    let seed = now + (id as i64) * 17 + (round as i64) * 31 + salt;
    20 + seed.rem_euclid(30)
}

fn locked_print(lock: &Semaphore, message: &str) {
    lock.wait();
    println!("{}", message);
    lock.signal();
}

fn locked_print_state(lock: &Semaphore, scenario: &str, id: usize, state: &str, round: usize) {
    lock.wait();
    println!("{} philosopher {} {} round {}", scenario, id, state, round);
    lock.signal();
}

fn locked_print_chopstick(
    lock: &Semaphore,
    scenario: &str,
    id: usize,
    action: &str,
    chopstick: usize,
    round: usize,
) {
    lock.wait();
    println!(
        "{} philosopher {} {} chopstick {} round {}",
        scenario, id, action, chopstick, round
    );
    lock.signal();
}

fn locked_print_done(lock: &Semaphore, scenario: &str, id: usize) {
    lock.wait();
    println!("{} philosopher {} finished", scenario, id);
    lock.signal();
}

entry!(main);
