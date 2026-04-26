#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;

use uefi::{Status, entry, mem::memory_map::MemoryMap};
use x86_64::registers::control::*;
use ysos_boot::*;

mod config;

const CONFIG_PATH: &str = "\\EFI\\BOOT\\boot.conf";

fn encode_log_level(log_level: &str) -> [u8; 16] {
    let mut encoded = [0u8; 16];
    let bytes = log_level.as_bytes();
    let len = core::cmp::min(bytes.len(), encoded.len().saturating_sub(1));
    encoded[..len].copy_from_slice(&bytes[..len]);
    encoded
}

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().expect("Failed to initialize utilities");

    log::set_max_level(log::LevelFilter::Info);
    info!("Running UEFI bootloader...");

    // 1. Load config
    let config = {
        /* FIXME: Load config file as Config */
        let mut file = open_file(CONFIG_PATH);
        let config_buf = load_file(&mut file);
        config::Config::parse(config_buf)
    };

    info!("Config: {:#x?}", config);

    // 2. Load ELF files
    let elf = {
        /* FIXME: Load kernel elf file */
        let mut elf_file = open_file(config.kernel_path);
        let elf_buf = load_file(&mut elf_file);
        xmas_elf::ElfFile::new(elf_buf).expect("Failed to parse kernel ELF")
    };

    set_entry(elf.header.pt2.entry_point() as usize);

    let apps = if config.load_apps {
        info!("Loading apps...");
        Some(load_apps())
    } else {
        info!("Skip loading apps");
        None
    };

    // 3. Load MemoryMap
    let mmap = uefi::boot::memory_map(MemoryType::LOADER_DATA).expect("Failed to get memory map");

    let max_phys_addr = mmap
        .entries()
        .map(|m| m.phys_start + m.page_count * 0x1000)
        .max()
        .unwrap()
        .max(0x1_0000_0000); // include IOAPIC MMIO area

    // 4. Map ELF segments, kernel stack and physical memory to virtual memory
    let mut page_table = current_page_table();
    let mut frame_allocator = UEFIFrameAllocator;

    // FIXME: root page table is readonly, disable write protect (Cr0)
    unsafe {
        Cr0::update(|cr0| {
            cr0.remove(Cr0Flags::WRITE_PROTECT);
        });
    }

    // FIXME: map physical memory to specific virtual address offset
    elf::map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut frame_allocator,
    );

    // FIXME: load and map the kernel elf file
    elf::load_elf(
        &elf,
        config.physical_memory_offset,
        &mut page_table,
        &mut frame_allocator,
        false,
    )
    .expect("Failed to load kernel ELF");

    // FIXME: map kernel stack
    let init_stack_pages = if config.kernel_stack_auto_grow == 0 {
        config.kernel_stack_size
    } else {
        config.kernel_stack_auto_grow
    };
    elf::map_range(
        config.kernel_stack_address,
        init_stack_pages,
        &mut page_table,
        &mut frame_allocator,
        false,
    )
    .expect("Failed to map kernel stack");

    // FIXME: recover write protect (Cr0)
    unsafe {
        Cr0::update(|cr0| {
            cr0.insert(Cr0Flags::WRITE_PROTECT);
        });
    }

    free_elf(elf);

    // 5. Pass system table to kernel
    let ptr = uefi::table::system_table_raw().expect("Failed to get system table");
    let system_table = ptr.cast::<core::ffi::c_void>();

    // 6. Exit boot and jump to ELF entry
    info!("Exiting boot services...");

    let mmap = unsafe { uefi::boot::exit_boot_services(None) };
    // NOTE: alloc & log are no longer available

    // construct BootInfo
    let bootinfo = BootInfo {
        memory_map: mmap.entries().copied().collect(),
        physical_memory_offset: config.physical_memory_offset,
        system_table,
        log_level: encode_log_level(config.log_level),
        loaded_apps: apps,
    };

    // align stack to 8 bytes
    let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000 - 8;

    jump_to_entry(&bootinfo, stacktop);
}
