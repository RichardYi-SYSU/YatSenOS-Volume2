#![no_std]
#![no_main]

use core::slice;

use lib::*;

extern crate lib;

const PAGE_SIZE: usize = 4096;
const HEAP_SIZE: usize = 8 * 1024 - 8;
const HALF_HEAP_END_OFFSET: usize = PAGE_SIZE - 8;
const FULL_HEAP_END_OFFSET: usize = HEAP_SIZE;
const WORDS_PER_FULL_HEAP: usize = HEAP_SIZE / core::mem::size_of::<u64>();

fn main() -> isize {
    let heap_start = sys_brk(None).expect("failed to query heap start");
    let half_heap_end = heap_start + HALF_HEAP_END_OFFSET;
    let full_heap_end = heap_start + FULL_HEAP_END_OFFSET;

    println!("Initial heap end: {:#x}", heap_start);
    assert_eq!(heap_start, sys_brk(None).unwrap());

    let ret = sys_brk(Some(full_heap_end)).expect("failed to grow heap");
    assert_eq!(ret, full_heap_end);
    assert_eq!(sys_brk(None).unwrap(), full_heap_end);
    println!("Grow heap to two pages: {:#x}", full_heap_end);

    let heap = unsafe { slice::from_raw_parts_mut(heap_start as *mut u64, WORDS_PER_FULL_HEAP) };
    let points = [
        (0usize, 0x1111_2222_3333_4444u64),
        (1usize, 0x5555_6666_7777_8888u64),
        (511usize, 0x9999_aaaa_bbbb_ccccu64),
        (512usize, 0xdddd_eeee_ffff_0001u64),
        (513usize, 0x1234_5678_9abc_def0u64),
        (1022usize, 0x0fed_cba9_8765_4321u64),
    ];

    for (idx, value) in points {
        heap[idx] = value;
    }
    for (idx, value) in points {
        assert_eq!(heap[idx], value);
    }
    println!("Cross-page write/read passed.");

    let ret = sys_brk(Some(half_heap_end)).expect("failed to shrink heap");
    assert_eq!(ret, half_heap_end);
    assert_eq!(sys_brk(None).unwrap(), half_heap_end);
    assert_eq!(heap[0], 0x1111_2222_3333_4444u64);
    assert_eq!(heap[1], 0x5555_6666_7777_8888u64);
    assert_eq!(heap[511], 0x9999_aaaa_bbbb_ccccu64);
    println!("Shrink heap to one page: {:#x}", half_heap_end);

    let ret = sys_brk(Some(full_heap_end)).expect("failed to regrow heap");
    assert_eq!(ret, full_heap_end);
    assert_eq!(sys_brk(None).unwrap(), full_heap_end);
    heap[512] = 0x2222_4444_6666_8888u64;
    heap[1022] = 0x1357_9bdf_2468_ace0u64;
    assert_eq!(heap[512], 0x2222_4444_6666_8888u64);
    assert_eq!(heap[1022], 0x1357_9bdf_2468_ace0u64);
    println!("Regrow heap and second-page write/read passed.");

    let ret = sys_brk(Some(heap_start)).expect("failed to release heap");
    assert_eq!(ret, heap_start);
    assert_eq!(sys_brk(None).unwrap(), heap_start);
    println!("Release heap to base: {:#x}", heap_start);
    println!("brk test passed.");

    0
}

entry!(main);
