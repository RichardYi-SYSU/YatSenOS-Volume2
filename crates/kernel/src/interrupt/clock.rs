use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use super::consts::*;
use crate::{memory::gdt, proc::ProcessContext};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Timer as u8]
        .set_handler_fn(clock_handler)
        .set_stack_index(gdt::CLOCK_IST_INDEX);
}

pub extern "C" fn clock(mut context: ProcessContext) {
    super::ack();
    crate::proc::switch(&mut context);
}

as_handler!(clock);
