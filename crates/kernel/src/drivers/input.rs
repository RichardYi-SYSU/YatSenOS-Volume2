use alloc::string::String;
use core::hint::spin_loop;

use crossbeam_queue::ArrayQueue;
use x86_64::instructions::interrupts;

use crate::drivers::serial::get_serial;

type Key = u8;

lazy_static! {
    static ref INPUT_BUF: ArrayQueue<Key> = ArrayQueue::new(128);
}

#[inline]
pub fn push_key(key: Key) {
    if INPUT_BUF.push(key).is_err() {
        warn!("Input buffer is full. Dropping key '{:?}'", key);
    }
}

#[inline]
pub fn try_pop_key() -> Option<Key> {
    INPUT_BUF.pop()
}

#[inline]
pub fn pop_key() -> Key {
    loop {
        if let Some(key) = try_pop_key() {
            return key;
        }
        spin_loop();
    }
}

pub fn get_line() -> String {
    let mut line = String::with_capacity(128);

    loop {
        match pop_key() {
            b'\n' => {
                println!();
                return line;
            }
            0x08 | 0x7f => {
                if line.pop().is_some() {
                    interrupts::without_interrupts(|| {
                        if let Some(mut serial) = get_serial() {
                            serial.backspace();
                        }
                    });
                }
            }
            key => {
                line.push(key as char);
                print!("{}", key as char);
            }
        }
    }
}
