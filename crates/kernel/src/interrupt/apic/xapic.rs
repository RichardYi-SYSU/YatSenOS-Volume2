use core::{
    fmt::{Debug, Error, Formatter},
    ptr::{read_volatile, write_volatile},
};

use bit_field::BitField;
use x86::cpuid::CpuId;

use super::super::consts::{Interrupts, Irq};
use super::LocalApic;

/// Default physical address of xAPIC
pub const LAPIC_ADDR: u64 = 0xFEE00000;

const REG_ID: u32 = 0x020;
const REG_VERSION: u32 = 0x030;
const REG_TPR: u32 = 0x080;
const REG_EOI: u32 = 0x0B0;
const REG_SPIV: u32 = 0x0F0;
const REG_ESR: u32 = 0x280;
const REG_ICR_LOW: u32 = 0x300;
const REG_ICR_HIGH: u32 = 0x310;
const REG_LVT_TIMER: u32 = 0x320;
const REG_LVT_PCINT: u32 = 0x340;
const REG_LVT_LINT0: u32 = 0x350;
const REG_LVT_LINT1: u32 = 0x360;
const REG_LVT_ERROR: u32 = 0x370;
const REG_INITIAL_COUNT: u32 = 0x380;
const REG_DIVIDE_CONFIG: u32 = 0x3E0;

const APIC_READ_AFTER_WRITE_REG: u32 = REG_ID;
const APIC_TIMER_DIVIDE_BY_1: u32 = 0b1011;
const APIC_TIMER_INITIAL_COUNT: u32 = 0x20000;

bitflags! {
    struct SpuriousInterruptFlags: u32 {
        const APIC_SOFTWARE_ENABLE = 1 << 8;
    }
}

bitflags! {
    struct LvtFlags: u32 {
        const MASKED = 1 << 16;
        const TIMER_PERIODIC = 1 << 17;
    }
}

bitflags! {
    struct InterruptCommandFlags: u64 {
        const BROADCAST = 1 << 19;
        const INIT = 5 << 8;
        const LEVEL = 1 << 15;
    }
}

pub struct XApic {
    addr: u64,
}

impl XApic {
    pub unsafe fn new(addr: u64) -> Self {
        XApic { addr }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.addr + reg as u64) as *const u32) }
    }

    unsafe fn write(&mut self, reg: u32, value: u32) {
        unsafe {
            write_volatile((self.addr + reg as u64) as *mut u32, value);
            self.read(APIC_READ_AFTER_WRITE_REG);
        }
    }
}

impl LocalApic for XApic {
    /// If this type APIC is supported
    fn support() -> bool {
        // FIXME: Check CPUID to see if xAPIC is supported.
        CpuId::new()
            .get_feature_info()
            .map(|f| f.has_apic())
            .unwrap_or(false)
    }

    /// Initialize the xAPIC for the current CPU.
    fn cpu_init(&mut self) {
        unsafe {
            // FIXME: Enable local APIC; set spurious interrupt vector.
            let mut spiv = self.read(REG_SPIV);
            spiv |= SpuriousInterruptFlags::APIC_SOFTWARE_ENABLE.bits();
            spiv &= !0xFF;
            spiv |= Interrupts::IrqBase as u32 + Irq::Spurious as u32;
            self.write(REG_SPIV, spiv);

            // FIXME: The timer repeatedly counts down at bus frequency
            self.write(REG_DIVIDE_CONFIG, APIC_TIMER_DIVIDE_BY_1);
            self.write(REG_INITIAL_COUNT, APIC_TIMER_INITIAL_COUNT);

            let mut lvt_timer = self.read(REG_LVT_TIMER);
            lvt_timer &= !0xFF;
            lvt_timer |= Interrupts::IrqBase as u32 + Irq::Timer as u32;
            lvt_timer &= !LvtFlags::MASKED.bits();
            lvt_timer |= LvtFlags::TIMER_PERIODIC.bits();
            self.write(REG_LVT_TIMER, lvt_timer);

            // FIXME: Disable logical interrupt lines (LINT0, LINT1)
            self.write(REG_LVT_LINT0, LvtFlags::MASKED.bits());
            self.write(REG_LVT_LINT1, LvtFlags::MASKED.bits());

            // FIXME: Disable performance counter overflow interrupts (PCINT)
            self.write(REG_LVT_PCINT, LvtFlags::MASKED.bits());

            // FIXME: Map error interrupt to IRQ_ERROR.
            let mut lvt_error = self.read(REG_LVT_ERROR);
            lvt_error &= !0xFF;
            lvt_error |= Interrupts::IrqBase as u32 + Irq::Error as u32;
            lvt_error &= !LvtFlags::MASKED.bits();
            self.write(REG_LVT_ERROR, lvt_error);

            // FIXME: Clear error status register (requires back-to-back
            // writes).
            self.write(REG_ESR, 0);
            self.write(REG_ESR, 0);

            // FIXME: Ack any outstanding interrupts.
            self.write(REG_EOI, 0);

            // FIXME: Send an Init Level De-Assert to synchronise arbitration
            // ID's.
            self.set_icr(
                (InterruptCommandFlags::BROADCAST
                    | InterruptCommandFlags::INIT
                    | InterruptCommandFlags::LEVEL)
                    .bits(),
            );

            // FIXME: Enable interrupts on the APIC (but not on the processor).
            self.write(REG_TPR, 0);
        }

        // NOTE: Try to use bitflags! macro to set the flags.
    }

    fn id(&self) -> u32 {
        // NOTE: Maybe you can handle regs like `0x0300` as a const.
        unsafe { self.read(REG_ID) >> 24 }
    }

    fn version(&self) -> u32 {
        unsafe { self.read(REG_VERSION) }
    }

    fn icr(&self) -> u64 {
        unsafe { (self.read(REG_ICR_HIGH) as u64) << 32 | self.read(REG_ICR_LOW) as u64 }
    }

    fn set_icr(&mut self, value: u64) {
        unsafe {
            while self.read(REG_ICR_LOW).get_bit(12) {}
            self.write(REG_ICR_HIGH, (value >> 32) as u32);
            self.write(REG_ICR_LOW, value as u32);
            while self.read(REG_ICR_LOW).get_bit(12) {}
        }
    }

    fn eoi(&mut self) {
        unsafe {
            self.write(REG_EOI, 0);
        }
    }
}

impl Debug for XApic {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.debug_struct("Xapic")
            .field("id", &self.id())
            .field("version", &self.version())
            .field("icr", &self.icr())
            .finish()
    }
}
