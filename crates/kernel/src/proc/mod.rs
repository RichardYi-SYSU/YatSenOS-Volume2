mod context;
mod data;
mod manager;
mod paging;
mod pid;
mod process;
mod processor;
mod sync;
mod vm;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

pub use context::ProcessContext;
pub use data::ProcessData;
pub use manager::*;
pub use paging::PageTableContext;
pub use pid::ProcessId;
use process::*;
pub use sync::{SemaphoreResult, SemaphoreSet};
pub use vm::ProcessVm;
use crate::drivers::filesystem;
use storage::FileSystem;
use x86_64::{VirtAddr, structures::idt::PageFaultErrorCode};
use xmas_elf::ElfFile;

pub const KERNEL_PID: ProcessId = ProcessId(1);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init(boot_info: &'static boot::BootInfo) {
    let proc_vm = ProcessVm::new(PageTableContext::new()).init_kernel_vm();

    trace!("Init kernel vm: {:#?}", proc_vm);

    // kernel process
    /* FIXME: create kernel process */
    let kproc = Process::new(
        String::from("kernel"),
        None,
        Some(proc_vm),
        Some(ProcessData::new()),
    );
    let app_list = boot_info.loaded_apps.as_ref();
    manager::init(kproc, app_list);

    info!("Process Manager Initialized.");
}

pub fn list_app() {
    let apps = match filesystem::get_rootfs().read_dir("/APP") {
        Ok(iter) => iter
            .filter(|meta| meta.is_file())
            .map(|meta| meta.name)
            .collect::<Vec<_>>()
            .join(", "),
        Err(err) => {
            println!("[!] Failed to list /APP: {:?}", err);
            return;
        }
    };

    if apps.is_empty() {
        println!("[!] No app found in /APP!");
    } else {
        println!("[+] App list: {}", apps);
    }
}

pub fn spawn(path: &str) -> Option<ProcessId> {
    let name = path.rsplit('/').find(|part| !part.is_empty())?;

    let mut file = match filesystem::get_rootfs().open_file(path) {
        Ok(file) => file,
        Err(err) => {
            warn!("Failed to open app {}: {:?}", path, err);
            return None;
        }
    };

    let mut elf_buf = Vec::new();
    if let Err(err) = file.read_all(&mut elf_buf) {
        warn!("Failed to read app {}: {:?}", path, err);
        return None;
    }

    let elf = match ElfFile::new(&elf_buf) {
        Ok(elf) => elf,
        Err(err) => {
            warn!("Failed to parse app {} as ELF: {:?}", path, err);
            return None;
        }
    };

    elf_spawn(name.to_string(), &elf)
}

pub fn elf_spawn(name: String, elf: &ElfFile) -> Option<ProcessId> {
    let pid = x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let process_name = name.to_lowercase();
        let parent = alloc::sync::Arc::downgrade(&manager.current());
        let pid = manager.spawn(elf, name, Some(parent), None);

        debug!("Spawned process: {}#{}", process_name, pid);
        pid
    });

    Some(pid)
}

pub fn fork(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let parent = manager.current().pid();

        manager.save_current(context);
        let child = manager.fork();

        manager.push_ready(parent);
        manager.push_ready(child);
        manager.switch_next(context);
    });
}

pub fn switch(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let pm = get_process_manager();
        let current = pm.current();
        let current_pid = current.pid();
        // save current process's context
        pm.save_current(context);
        // handle ready queue update
        if current.read().status() == ProgramStatus::Ready {
            pm.push_ready(current_pid);
        }
        // restore next process's context
        pm.switch_next(context);
    });
}

pub fn spawn_kernel_thread(entry: fn() -> !, name: String, data: Option<ProcessData>) -> ProcessId {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let entry = VirtAddr::new(entry as usize as u64);
        get_process_manager().spawn_kernel_thread(entry, name, data)
    })
}

pub fn print_process_list() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().print_process_list();
    })
}

pub fn read(fd: u8, buf: &mut [u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().read(fd, buf))
}

pub fn write(fd: u8, buf: &[u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().write(fd, buf))
}

pub fn open(resource: crate::utils::Resource) -> u8 {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().open(resource))
}

pub fn close(fd: u8) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().close(fd))
}

pub fn get_pid() -> ProcessId {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().current().pid())
}

pub fn env(key: &str) -> Option<String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // FIXME: get current process's environment variable
        get_process_manager().current().read().env(key)
    })
}

pub fn exit_code(pid: ProcessId) -> Option<isize> {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().exit_code(pid))
}

#[inline]
pub fn still_alive(pid: ProcessId) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().is_alive(pid))
}

pub fn wait_pid(pid: ProcessId, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let current = manager.current().pid();

        if let Some(code) = manager.exit_code(pid) {
            context.set_rax(code as usize);
        } else if pid == current || !manager.is_alive(pid) {
            context.set_rax((-1isize) as usize);
        } else if manager.is_alive(pid) {
            manager.wait_pid(pid);
            manager.save_current(context);
            manager.block(current);
            manager.switch_next(context);
        }
    })
}

pub fn exit(ret: isize, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        manager.kill_current(ret);
        manager.switch_next(context);
    });
}

pub fn process_exit(ret: isize) -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().kill_current(ret);
    });

    loop {
        x86_64::instructions::hlt();
    }
}

pub fn handle_page_fault(addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().handle_page_fault(addr, err_code)
    })
}
