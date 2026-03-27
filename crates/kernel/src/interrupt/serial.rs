use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use super::{ack, consts::*};
use crate::drivers::{
    input::push_key,
    serial::get_serial,
};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Serial0 as u8].set_handler_fn(serial_handler);
}

pub extern "x86-interrupt" fn serial_handler(_sf: InterruptStackFrame) {
    receive();
    ack();
}

/// Receive character from uart 16550.
/// Should be called on every serial interrupt.
fn receive() {
    // FIXME: receive character from uart 16550, put it into INPUT_BUFFER
    if let Some(mut serial) = get_serial() {
        if let Some(key) = serial.receive() {
            push_key(key);
        }
    }
}
