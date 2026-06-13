use x86_64::{
    VirtAddr,
    registers::rflags::RFlags,
    structures::{gdt::SegmentSelector, idt::InterruptStackFrameValue},
};

use crate::{
    RegistersValue,
    memory::gdt::{get_selector, get_user_selector},
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcessContextValue {
    pub regs: RegistersValue,
    pub stack_frame: InterruptStackFrameValue,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ProcessContext {
    value: ProcessContextValue,
}

impl ProcessContext {
    #[inline]
    pub fn set_rax(&mut self, value: usize) {
        self.value.regs.rax = value;
    }

    #[inline]
    pub fn set_stack_pointer(&mut self, value: VirtAddr) {
        self.value.stack_frame.stack_pointer = value;
    }

    #[inline]
    pub fn stack_pointer(&self) -> VirtAddr {
        self.value.stack_frame.stack_pointer
    }

    #[inline]
    pub fn move_stack_down(&mut self, offset: u64) {
        self.value.stack_frame.stack_pointer -= offset;

        let offset = offset as usize;
        if self.value.regs.rbp >= offset {
            self.value.regs.rbp -= offset;
        }
    }

    #[inline]
    pub fn save(&mut self, context: &ProcessContext) {
        self.value = context.value;
    }

    #[inline]
    pub fn restore(&self, context: &mut ProcessContext) {
        context.value = self.value;
    }

    pub fn init_stack_frame(&mut self, entry: VirtAddr, stack_top: VirtAddr) {
        self.value.stack_frame.stack_pointer = stack_top;
        self.value.stack_frame.instruction_pointer = entry;
        self.value.stack_frame.cpu_flags =
            RFlags::IOPL_HIGH | RFlags::IOPL_LOW | RFlags::INTERRUPT_FLAG;

        let selector = get_user_selector();
        self.value.stack_frame.code_segment = selector.user_code_selector;
        self.value.stack_frame.stack_segment = selector.user_data_selector;

        trace!("Init stack frame: {:#?}", &self.stack_frame);
    }

    pub fn init_kernel_stack_frame(&mut self, entry: VirtAddr, stack_top: VirtAddr) {
        self.value.stack_frame.stack_pointer = stack_top;
        self.value.stack_frame.instruction_pointer = entry;
        self.value.stack_frame.cpu_flags = RFlags::INTERRUPT_FLAG;

        let selector = get_selector();
        self.value.stack_frame.code_segment = selector.code_selector;
        self.value.stack_frame.stack_segment = selector.data_selector;

        trace!("Init kernel stack frame: {:#?}", &self.stack_frame);
    }
}

impl Default for ProcessContextValue {
    fn default() -> Self {
        Self {
            regs: RegistersValue::default(),
            stack_frame: InterruptStackFrameValue::new(
                VirtAddr::new(0x1000),
                SegmentSelector(0),
                RFlags::empty(),
                VirtAddr::new(0x2000),
                SegmentSelector(0),
            ),
        }
    }
}

impl core::ops::Deref for ProcessContext {
    type Target = ProcessContextValue;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl core::fmt::Debug for ProcessContext {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.value.fmt(f)
    }
}

impl core::fmt::Debug for ProcessContextValue {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut f = f.debug_struct("StackFrame");
        f.field("stack_top", &self.stack_frame.stack_pointer);
        f.field("cpu_flags", &self.stack_frame.cpu_flags);
        f.field("instruction_pointer", &self.stack_frame.instruction_pointer);
        f.field("regs", &self.regs);
        f.finish()
    }
}
