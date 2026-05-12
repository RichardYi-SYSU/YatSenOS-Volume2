#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const PRODUCER_COUNT: usize = 8;
const CONSUMER_COUNT: usize = 8;
const WORKER_COUNT: usize = PRODUCER_COUNT + CONSUMER_COUNT;
const MESSAGE_PER_WORKER: usize = 10;
const MAX_CAPACITY: usize = 16;

const SEM_MUTEX_KEY: u32 = 0x200;
const SEM_EMPTY_KEY: u32 = 0x201;
const SEM_FULL_KEY: u32 = 0x202;

static MUTEX: Semaphore = Semaphore::new(SEM_MUTEX_KEY);
static EMPTY: Semaphore = Semaphore::new(SEM_EMPTY_KEY);
static FULL: Semaphore = Semaphore::new(SEM_FULL_KEY);

static mut QUEUE: [usize; MAX_CAPACITY] = [0; MAX_CAPACITY];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;
static mut COUNT: usize = 0;
static mut CAPACITY: usize = 0;

fn main() -> isize {
    for capacity in [1, 4, 8, 16] {
        run_queue_test(capacity);
    }

    0
}

fn run_queue_test(capacity: usize) {
    reset_queue(capacity);

    MUTEX.init(1);
    EMPTY.init(capacity);
    FULL.init(0);

    println!("=== mq capacity {} ===", capacity);

    let mut pids = [0u16; WORKER_COUNT];

    for i in 0..WORKER_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            if i < PRODUCER_COUNT {
                producer(i);
            } else {
                consumer(i - PRODUCER_COUNT);
            }
            sys_exit(0);
        } else {
            pids[i] = pid;
        }
    }

    sys_stat();

    for pid in pids {
        sys_wait_pid(pid);
    }

    let final_count = unsafe { COUNT };
    println!("capacity {} final queue count: {}", capacity, final_count);

    MUTEX.remove();
    EMPTY.remove();
    FULL.remove();
}

fn producer(id: usize) {
    for seq in 0..MESSAGE_PER_WORKER {
        let message = id * 100 + seq;

        EMPTY.wait();
        MUTEX.wait();
        push_message(id, message);
        MUTEX.signal();
        FULL.signal();
    }
}

fn consumer(id: usize) {
    for _ in 0..MESSAGE_PER_WORKER {
        FULL.wait();
        MUTEX.wait();
        pop_message(id);
        MUTEX.signal();
        EMPTY.signal();
    }
}

fn push_message(id: usize, message: usize) {
    unsafe {
        QUEUE[TAIL] = message;
        TAIL = (TAIL + 1) % CAPACITY;
        COUNT += 1;
        let count = COUNT;
        let capacity = CAPACITY;

        println!(
            "producer {} -> {:03}, count = {}/{}",
            id, message, count, capacity
        );
    }
}

fn pop_message(id: usize) {
    unsafe {
        let message = QUEUE[HEAD];
        HEAD = (HEAD + 1) % CAPACITY;
        COUNT -= 1;
        let count = COUNT;
        let capacity = CAPACITY;

        println!(
            "consumer {} <- {:03}, count = {}/{}",
            id, message, count, capacity
        );
    }
}

fn reset_queue(capacity: usize) {
    unsafe {
        HEAD = 0;
        TAIL = 0;
        COUNT = 0;
        CAPACITY = capacity;
        QUEUE = [0; MAX_CAPACITY];
    }
}

entry!(main);
