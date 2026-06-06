use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::{
    VirtAddr, align_up,
    structures::paging::{Page, Size4KiB, mapper::UnmapError},
};

use super::{FrameAllocatorRef, MapperRef};
use crate::memory::PAGE_SIZE;

// user process runtime heap
// 0x100000000 bytes -> 4GiB
// from 0x0000_2000_0000_0000 to 0x0000_2000_ffff_fff8
pub const HEAP_START: u64 = 0x2000_0000_0000;
pub const HEAP_PAGES: u64 = 0x100000;
pub const HEAP_SIZE: u64 = HEAP_PAGES * crate::memory::PAGE_SIZE;
pub const HEAP_END: u64 = HEAP_START + HEAP_SIZE - 8;

/// User process runtime heap
///
/// always page aligned, the range is [base, end)
pub struct Heap {
    /// the base address of the heap
    ///
    /// immutable after initialization
    base: VirtAddr,

    /// the current end address of the heap
    ///
    /// use atomic to allow multiple threads to access the heap
    end: Arc<AtomicU64>,
}

impl Heap {
    pub fn empty() -> Self {
        Self {
            base: VirtAddr::new(HEAP_START),
            end: Arc::new(AtomicU64::new(HEAP_START)),
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            base: self.base,
            end: self.end.clone(),
        }
    }

    pub fn brk(
        &self,
        new_end: Option<VirtAddr>,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Option<VirtAddr> {
        let old_end = self.end.load(Ordering::Relaxed);
        let Some(new_end) = new_end else {
            return Some(VirtAddr::new(old_end));
        };

        let new_end = new_end.as_u64();
        if new_end < self.base.as_u64() || new_end > HEAP_END {
            return None;
        }

        let old_map_end = self.aligned_map_end(old_end);
        let new_map_end = self.aligned_map_end(new_end);

        if new_map_end > old_map_end {
            let count = (new_map_end - old_map_end) / PAGE_SIZE;
            elf::map_range(old_map_end, count, mapper, alloc, true).ok()?;
        } else if new_map_end < old_map_end {
            let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(new_map_end));
            let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(old_map_end - 1));
            elf::unmap_range(Page::range(start_page, end_page + 1), mapper, alloc).ok()?;
        }

        self.end.store(new_end, Ordering::Relaxed);
        Some(VirtAddr::new(new_end))
    }

    fn aligned_map_end(&self, end: u64) -> u64 {
        if end == self.base.as_u64() {
            self.base.as_u64()
        } else {
            align_up(end, PAGE_SIZE)
        }
    }

    pub(super) fn clean_up(
        &self,
        mapper: MapperRef,
        dealloc: FrameAllocatorRef,
    ) -> Result<(), UnmapError> {
        if self.memory_usage() == 0 {
            return Ok(());
        }

        // load the current end address and **reset it to base** (use `swap`)
        let old_end = self.end.swap(self.base.as_u64(), Ordering::Relaxed);
        let start_page = Page::<Size4KiB>::containing_address(self.base);
        let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(old_end - 1));

        // unmap the heap pages
        elf::unmap_range(Page::range(start_page, end_page + 1), mapper, dealloc)?;

        Ok(())
    }

    pub fn memory_usage(&self) -> u64 {
        self.end.load(Ordering::Relaxed) - self.base.as_u64()
    }
}

impl core::fmt::Debug for Heap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Heap")
            .field("base", &format_args!("{:#x}", self.base.as_u64()))
            .field(
                "end",
                &format_args!("{:#x}", self.end.load(Ordering::Relaxed)),
            )
            .finish()
    }
}
