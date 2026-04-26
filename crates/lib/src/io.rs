use alloc::{
    string::{String, ToString},
};

use crate::*;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdin {
    fn new() -> Self {
        Self
    }

    pub fn read_line(&self) -> String {
        // allocate string
        // read from input buffer
        //- maybe char by char?
        // handle backspace / enter...
        // return string
        let mut line = String::new();
        let mut buf = [0u8; 1];

        loop {
            let Some(count) = sys_read(0, &mut buf) else {
                continue;
            };

            if count == 0 {
                core::hint::spin_loop();
                continue;
            }

            match buf[0] {
                b'\n' | b'\r' => {
                    stdout().write("\n");
                    return line;
                }
                0x08 | 0x7f => {
                    if line.pop().is_some() {
                        stdout().write("\x08 \x08");
                    }
                }
                ch => {
                    line.push(ch as char);
                    let s = core::str::from_utf8(&buf[..1]).unwrap_or("");
                    stdout().write(s);
                }
            }
        }
    }
}

impl Stdout {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(1, s.as_bytes());
    }
}

impl Stderr {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(2, s.as_bytes());
    }
}

pub fn stdin() -> Stdin {
    Stdin::new()
}

pub fn stdout() -> Stdout {
    Stdout::new()
}

pub fn stderr() -> Stderr {
    Stderr::new()
}
