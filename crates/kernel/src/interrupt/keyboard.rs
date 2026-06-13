use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::Mutex;
use x86_64::{
    instructions::port::Port,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame},
};

use super::{ack, consts::*};
use crate::drivers::input::push_key;

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ));
}

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Keyboard as u8].set_handler_fn(keyboard_handler);
}

pub extern "x86-interrupt" fn keyboard_handler(_sf: InterruptStackFrame) {
    receive();
    ack();
}

fn receive() {
    let scancode = unsafe {
        let mut port = Port::<u8>::new(0x60);
        port.read()
    };

    let mut keyboard = KEYBOARD.lock();
    let Ok(Some(event)) = keyboard.add_byte(scancode) else {
        return;
    };
    let Some(key) = keyboard.process_keyevent(event) else {
        return;
    };

    match key {
        DecodedKey::Unicode('\n') | DecodedKey::Unicode('\r') => push_key(b'\n'),
        DecodedKey::Unicode(ch) if ch.is_ascii() => push_key(ch as u8),
        DecodedKey::RawKey(pc_keyboard::KeyCode::Backspace) => push_key(0x08),
        _ => {}
    }
}
