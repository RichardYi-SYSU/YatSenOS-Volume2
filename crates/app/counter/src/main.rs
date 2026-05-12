#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const THREAD_COUNT: usize = 8;
const INC_PER_THREAD: usize = 100;
const SEM_KEY: u32 = 0x100;

static SPIN_LOCK: SpinLock = SpinLock::new();
static SEM: Semaphore = Semaphore::new(SEM_KEY);

static mut COUNTER: isize = 0;

fn main() -> isize {
    test_spin();
    reset_counter();

    let pid = sys_fork();
    if pid == 0 {
        test_semaphore();
        sys_exit(0);
    } else {
        sys_wait_pid(pid);
    }

    0
}

fn test_spin() {
    let mut pids = [0u16; THREAD_COUNT];

    for i in 0..THREAD_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            do_spin_counter_inc();
            sys_exit(0);
        } else {
            pids[i] = pid; // only parent knows child's pid
        }
    }

    let cpid = sys_get_pid();
    println!("process #{} holds SpinLock threads: {:?}", cpid, &pids);
    sys_stat();

    for i in 0..THREAD_COUNT {
        println!("#{} waiting for #{}...", cpid, pids[i]);
        sys_wait_pid(pids[i]);
    }

    println!("SpinLock COUNTER result: {}", unsafe { COUNTER });
}

fn test_semaphore() {
    SEM.init(1);

    let mut pids = [0u16; THREAD_COUNT];

    for i in 0..THREAD_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            do_sem_counter_inc();
            sys_exit(0);
        } else {
            pids[i] = pid; // only parent knows child's pid
        }
    }

    let cpid = sys_get_pid();
    println!("process #{} holds Semaphore threads: {:?}", cpid, &pids);
    sys_stat();

    for i in 0..THREAD_COUNT {
        println!("#{} waiting for #{}...", cpid, pids[i]);
        sys_wait_pid(pids[i]);
    }

    println!("Semaphore COUNTER result: {}", unsafe { COUNTER });
    SEM.remove();
}

fn do_spin_counter_inc() {
    for _ in 0..INC_PER_THREAD {
        SPIN_LOCK.acquire();
        inc_counter();
        SPIN_LOCK.release();
    }
}

fn do_sem_counter_inc() {
    for _ in 0..INC_PER_THREAD {
        SEM.wait();
        inc_counter();
        SEM.signal();
    }
}

fn reset_counter() {
    unsafe {
        COUNTER = 0;
    }
}

/// Increment the counter
///
/// this function simulate a critical section by delay
/// DO NOT MODIFY THIS FUNCTION
fn inc_counter() {
    unsafe {
        delay();
        let mut val = COUNTER;
        delay();
        val += 1;
        delay();
        COUNTER = val;
    }
}

#[inline(never)]
#[unsafe(no_mangle)]
fn delay() {
    for _ in 0..0x100 {
        core::hint::spin_loop();
    }
}

entry!(main);
