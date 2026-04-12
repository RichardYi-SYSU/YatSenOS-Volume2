mod context;
mod data;
mod manager;
mod paging;
mod pid;
mod process;
mod processor;
mod vm;

use alloc::string::String;

pub use context::ProcessContext;
pub use data::ProcessData;
use manager::*;
pub use paging::PageTableContext;
pub use pid::ProcessId;
use process::*;
pub use vm::ProcessVm;
use x86_64::{VirtAddr, structures::idt::PageFaultErrorCode};

pub const KERNEL_PID: ProcessId = ProcessId(1);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init() {
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
    manager::init(kproc);

    info!("Process Manager Initialized.");
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

pub fn env(key: &str) -> Option<String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // FIXME: get current process's environment variable
        get_process_manager().current().read().env(key)
    })
}

pub fn exit_code(pid: ProcessId) -> Option<isize> {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().exit_code(pid))
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
