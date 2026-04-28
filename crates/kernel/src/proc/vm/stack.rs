use x86_64::{
    VirtAddr,
    structures::paging::{Page, mapper::MapToError, page::*},
};

use super::{FrameAllocatorRef, MapperRef};

// 0xffff_ff00_0000_0000 is the kernel's address space
pub const STACK_MAX: u64 = 0x4000_0000_0000;
pub const STACK_MAX_PAGES: u64 = 0x100000;
pub const STACK_MAX_SIZE: u64 = STACK_MAX_PAGES * crate::memory::PAGE_SIZE;
pub const STACK_START_MASK: u64 = !(STACK_MAX_SIZE - 1);
// [bot..0x2000_0000_0000..top..0x3fff_ffff_ffff]
// init stack
pub const STACK_DEF_BOT: u64 = STACK_MAX - STACK_MAX_SIZE;
pub const STACK_DEF_PAGE: u64 = 1;
pub const STACK_DEF_SIZE: u64 = STACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const STACK_INIT_BOT: u64 = STACK_MAX - STACK_DEF_SIZE;
pub const STACK_INIT_TOP: u64 = STACK_MAX - 8;

const STACK_INIT_TOP_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(STACK_INIT_TOP));

// [bot..0xffffff0100000000..top..0xffffff01ffffffff]
// kernel stack
pub const KSTACK_MAX: u64 = 0xffff_ff02_0000_0000;
pub const KSTACK_DEF_BOT: u64 = KSTACK_MAX - STACK_MAX_SIZE;
pub const KSTACK_DEF_PAGE: u64 = 512;
pub const KSTACK_DEF_SIZE: u64 = KSTACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const KSTACK_INIT_BOT: u64 = KSTACK_MAX - KSTACK_DEF_SIZE;
pub const KSTACK_INIT_TOP: u64 = KSTACK_MAX - 8;

const KSTACK_INIT_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_INIT_BOT));
const KSTACK_INIT_TOP_PAGE: Page<Size4KiB> =
    Page::containing_address(VirtAddr::new(KSTACK_INIT_TOP));

pub struct Stack {
    range: PageRange<Size4KiB>,
    usage: u64,
}

impl Stack {
    pub fn new(top: Page, size: u64) -> Self {
        Self {
            range: Page::range(top - size + 1, top + 1),
            usage: size,
        }
    }

    pub const fn empty() -> Self {
        Self {
            range: Page::range(STACK_INIT_TOP_PAGE, STACK_INIT_TOP_PAGE),
            usage: 0,
        }
    }

    pub const fn kstack() -> Self {
        Self {
            range: Page::range(KSTACK_INIT_PAGE, KSTACK_INIT_TOP_PAGE),
            usage: KSTACK_DEF_PAGE,
        }
    }

    pub fn init(&mut self, bottom_addr: u64, mapper: MapperRef, alloc: FrameAllocatorRef) {
        debug_assert!(self.usage == 0, "Stack is not empty.");

        self.range = elf::map_range(bottom_addr, STACK_DEF_PAGE, mapper, alloc, true).unwrap();
        self.usage = STACK_DEF_PAGE;
    }

    pub fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> bool {
        if !self.is_on_stack(addr) {
            return false;
        }

        match self.grow_stack(addr, mapper, alloc) {
            Ok(handled) => handled,
            Err(m) => {
                error!("Grow stack failed: {:?}", m);
                false
            }
        }
    }

    fn is_on_stack(&self, addr: VirtAddr) -> bool {
        let addr = addr.as_u64();
        let cur_stack_bot = self.range.start.start_address().as_u64();
        trace!("Current stack bot: {:#x}", cur_stack_bot);
        trace!("Address to access: {:#x}", addr);
        addr & STACK_START_MASK == cur_stack_bot & STACK_START_MASK
    }

    fn grow_stack(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Result<bool, MapToError<Size4KiB>> {
        debug_assert!(self.is_on_stack(addr), "Address is not on stack.");
        // FIXME: grow stack for page fault
        let fault_page = Page::<Size4KiB>::containing_address(addr);
        let old_start = self.range.start;
        let old_end = self.range.end;

        if fault_page >= old_start {
            warn!(
                "Invalid stack growth request: fault={:#x}, current range={:#x}..{:#x}",
                addr,
                old_start.start_address().as_u64(),
                old_end.start_address().as_u64()
            );
            return Ok(false);
        }

        let page_count = old_start - fault_page;
        elf::map_range(fault_page.start_address().as_u64(), page_count, mapper, alloc, true)?;

        self.range = Page::range(fault_page, old_end);
        self.usage = self.range.end - self.range.start;

        info!(
            "Grow stack: new range={:#x}..{:#x}, pages={}",
            self.range.start.start_address().as_u64(),
            self.range.end.start_address().as_u64(),
            self.usage
        );

        Ok(true)
    }

    pub fn reclaim(&mut self, mapper: MapperRef, alloc: FrameAllocatorRef) {
        if self.usage == 0 {
            return;
        }

        let range = Page::range(self.range.start, self.range.end);
        elf::unmap_range(range, mapper, alloc).expect("Failed to reclaim process stack");

        let empty = Stack::empty();
        self.range = empty.range;
        self.usage = 0;
    }

    pub fn memory_usage(&self) -> u64 {
        self.usage * crate::memory::PAGE_SIZE
    }

    pub fn pages(&self) -> u64 {
        self.usage
    }
}

impl core::fmt::Debug for Stack {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Stack")
            .field(
                "top",
                &format_args!("{:#x}", self.range.end.start_address().as_u64()),
            )
            .field(
                "bot",
                &format_args!("{:#x}", self.range.start.start_address().as_u64()),
            )
            .finish()
    }
}
