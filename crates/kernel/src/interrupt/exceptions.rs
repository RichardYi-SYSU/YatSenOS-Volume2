use x86_64::{
    VirtAddr,
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::memory::*;

macro_rules! exception_handler {
    ($name:ident, $message:literal) => {
        pub extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            panic!(concat!("EXCEPTION: ", $message, "\n\n{:#?}"), stack_frame);
        }
    };
}

macro_rules! exception_handler_with_err_code {
    ($name:ident, $message:literal) => {
        pub extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame, error_code: u64) {
            panic!(
                concat!("EXCEPTION: ", $message, ", ERROR_CODE: 0x{:016x}\n\n{:#?}"),
                error_code,
                stack_frame
            );
        }
    };
}

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.debug.set_handler_fn(debug_handler);
    idt.non_maskable_interrupt
        .set_handler_fn(non_maskable_interrupt_handler);
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.overflow.set_handler_fn(overflow_handler);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range_exceeded_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.device_not_available
        .set_handler_fn(device_not_available_handler);
    idt.double_fault
        .set_handler_fn(double_fault_handler)
        .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    idt.invalid_tss.set_handler_fn(invalid_tss_handler);
    idt.segment_not_present
        .set_handler_fn(segment_not_present_handler);
    idt.stack_segment_fault
        .set_handler_fn(stack_segment_fault_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
    idt.page_fault
        .set_handler_fn(page_fault_handler)
        .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
    idt.x87_floating_point
        .set_handler_fn(x87_floating_point_handler);
    idt.alignment_check
        .set_handler_fn(alignment_check_handler);
    idt.machine_check.set_handler_fn(machine_check_handler);
    idt.simd_floating_point
        .set_handler_fn(simd_floating_point_handler);
    idt.virtualization.set_handler_fn(virtualization_handler);
    idt.cp_protection_exception
        .set_handler_fn(cp_protection_exception_handler);
    idt.hv_injection_exception
        .set_handler_fn(hv_injection_exception_handler);
    idt.vmm_communication_exception
        .set_handler_fn(vmm_communication_exception_handler);
    idt.security_exception
        .set_handler_fn(security_exception_handler);

    // TODO: you should handle more exceptions here
    // see: https://wiki.osdev.org/Exceptions
}

exception_handler!(divide_error_handler, "DIVIDE ERROR");
exception_handler!(debug_handler, "DEBUG");
exception_handler!(non_maskable_interrupt_handler, "NON MASKABLE INTERRUPT");
exception_handler!(breakpoint_handler, "BREAKPOINT");
exception_handler!(overflow_handler, "OVERFLOW");
exception_handler!(bound_range_exceeded_handler, "BOUND RANGE EXCEEDED");
exception_handler!(invalid_opcode_handler, "INVALID OPCODE");
exception_handler!(device_not_available_handler, "DEVICE NOT AVAILABLE");

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "EXCEPTION: DOUBLE FAULT, ERROR_CODE: 0x{:016x}\n\n{:#?}",
        error_code, stack_frame
    );
}

exception_handler_with_err_code!(invalid_tss_handler, "INVALID TSS");
exception_handler_with_err_code!(segment_not_present_handler, "SEGMENT NOT PRESENT");
exception_handler_with_err_code!(stack_segment_fault_handler, "STACK SEGMENT FAULT");
exception_handler_with_err_code!(
    general_protection_fault_handler,
    "GENERAL PROTECTION FAULT"
);

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    err_code: PageFaultErrorCode,
) {
    panic!(
        "EXCEPTION: PAGE FAULT, ERROR_CODE: {:?}\n\nTrying to access: {:#x}\n{:#?}",
        err_code,
        Cr2::read().unwrap_or(VirtAddr::new_truncate(0xdeadbeef)),
        stack_frame
    );
}

exception_handler!(x87_floating_point_handler, "X87 FLOATING POINT");
exception_handler_with_err_code!(alignment_check_handler, "ALIGNMENT CHECK");

pub extern "x86-interrupt" fn machine_check_handler(stack_frame: InterruptStackFrame) -> ! {
    panic!("EXCEPTION: MACHINE CHECK\n\n{:#?}", stack_frame);
}

exception_handler!(simd_floating_point_handler, "SIMD FLOATING POINT");
exception_handler!(virtualization_handler, "VIRTUALIZATION");
exception_handler_with_err_code!(
    cp_protection_exception_handler,
    "CONTROL PROTECTION EXCEPTION"
);
exception_handler!(hv_injection_exception_handler, "HYPERVISOR INJECTION EXCEPTION");
exception_handler_with_err_code!(
    vmm_communication_exception_handler,
    "VMM COMMUNICATION EXCEPTION"
);
exception_handler_with_err_code!(security_exception_handler, "SECURITY EXCEPTION");
