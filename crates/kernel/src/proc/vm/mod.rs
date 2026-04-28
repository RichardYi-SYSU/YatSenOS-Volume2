use alloc::format;

use x86_64::{
    VirtAddr,
    structures::paging::*,
};

use crate::{humanized_size, memory::*};

pub mod stack;

use self::stack::*;
use super::{PageTableContext, ProcessId};

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext,

    // stack is pre-process allocated
    pub(super) stack: Stack,

    // bonus 1: loaded user image pages
    pub(super) code_pages: u64,
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
            code_pages: 0,
        }
    }

    pub fn init_kernel_vm(mut self) -> Self {
        // TODO: record kernel code usage
        self.stack = Stack::kstack();
        self
    }

    pub fn init_proc_stack(&mut self, pid: ProcessId) -> VirtAddr {
        // FIXME: calculate the stack for pid
        let offset = (pid.0 as u64 - 1) * STACK_MAX_SIZE;
        let stack_bot_addr = STACK_INIT_BOT - offset;
        let stack_top_addr = VirtAddr::new(STACK_INIT_TOP - offset);

        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();
        self.stack.init(stack_bot_addr, mapper, alloc);

        stack_top_addr
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.handle_page_fault(addr, mapper, alloc)
    }

    pub fn load_elf(&mut self, elf: &xmas_elf::ElfFile) -> u64 {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        // load elf to process pagetable
        self.code_pages = elf::load_elf(
            elf,
            *PHYSICAL_OFFSET.get().unwrap(),
            mapper,
            alloc,
            true,
        )
        .expect("Failed to load user ELF");

        self.code_pages
    }

    pub fn reclaim_stack(&mut self) {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.reclaim(mapper, alloc);
    }

    pub(super) fn memory_usage(&self) -> u64 {
        self.stack.memory_usage() + self.code_pages * PAGE_SIZE
    }

    pub(super) fn memory_pages(&self) -> u64 {
        self.stack.pages() + self.code_pages
    }
}

impl core::fmt::Debug for ProcessVm {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (size, unit) = humanized_size(self.memory_usage());

        f.debug_struct("ProcessVm")
            .field("stack", &self.stack)
            .field("memory_usage", &format!("{} {}", size, unit))
            .field("page_table", &self.page_table)
            .finish()
    }
}
