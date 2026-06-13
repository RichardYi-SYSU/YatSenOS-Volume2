use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicUsize, Ordering},
};

use linked_list_allocator::LockedHeap;

use crate::*;

const PAGE_SIZE: usize = 4096;
const INITIAL_HEAP_SIZE: usize = 8 * 1024 - 8;
const MAX_HEAP_SIZE: usize = 8 * 1024 * 1024 - 8;


static HEAP_END: AtomicUsize = AtomicUsize::new(0);
static HEAP_LIMIT: AtomicUsize = AtomicUsize::new(0);

struct BrkAllocator {
    allocator: LockedHeap,
}

#[global_allocator]
static ALLOCATOR: BrkAllocator = BrkAllocator::empty();

pub fn init() {
    ALLOCATOR.init();
}

impl BrkAllocator {
    pub const fn empty() -> Self {
        Self {
            allocator: LockedHeap::empty(),
        }
    }

    pub fn init(&self) {
        // Initialize the user heap by asking the kernel to map the first heap
        // chunk.  LockedHeap can only manage memory after brk succeeds.
        let heap_start = sys_brk(None).expect("Failed to query heap start");
        let heap_end = heap_start + INITIAL_HEAP_SIZE;
        let heap_limit = heap_start + MAX_HEAP_SIZE;

        let ret = sys_brk(Some(heap_end)).expect("Failed to allocate heap");
        assert_eq!(ret, heap_end, "Failed to allocate heap");

        HEAP_END.store(heap_end, Ordering::Relaxed);
        HEAP_LIMIT.store(heap_limit, Ordering::Relaxed);

        unsafe {
            self.allocator
                .lock()
                .init(heap_start as *mut u8, INITIAL_HEAP_SIZE);
        }
    }

    pub unsafe fn extend(&self, layout: Layout) -> bool {
        // Extend at least enough for the failed allocation, but always grow by
        // full pages because brk maps/unmaps heap memory at page granularity.
        let heap_end = HEAP_END.load(Ordering::Relaxed);
        let heap_limit = HEAP_LIMIT.load(Ordering::Relaxed);

        if heap_end == 0 || heap_limit == 0 {
            return false;
        }

        let required = layout.size().max(layout.align()).max(PAGE_SIZE);
        let grow_size = align_up(required, PAGE_SIZE);
        let Some(new_heap_end) = heap_end.checked_add(grow_size) else {
            return false;
        };

        if new_heap_end > heap_limit {
            return false;
        }

        match sys_brk(Some(new_heap_end)) {
            Some(ret) if ret == new_heap_end => {
                // Only expose the new range to LockedHeap after the kernel has
                // accepted the new brk.  This keeps allocator metadata in sync
                // with the actually mapped user address space.
                unsafe {
                    self.allocator.lock().extend(grow_size);
                }
                HEAP_END.store(new_heap_end, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }
}

unsafe impl GlobalAlloc for BrkAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut ptr = unsafe { self.allocator.alloc(layout) };
        // A null pointer means LockedHeap has no suitable free block.  Grow the
        // process heap with brk once, then retry the same allocation.
        if ptr.is_null() && unsafe { self.extend(layout) } {
            ptr = unsafe { self.allocator.alloc(layout) };
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            self.allocator.dealloc(ptr, layout);
        }
    }
}

fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

#[allow(unexpected_cfgs)]
#[cfg(all(not(test), not(rust_analyzer)))]
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error: {:?}", layout)
}
