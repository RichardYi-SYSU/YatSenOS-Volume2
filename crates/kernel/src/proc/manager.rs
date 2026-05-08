use alloc::{collections::*, format, sync::Arc};

use hashbrown::HashMap;
use spin::{Mutex, RwLock};

use super::*;
use crate::memory::{PAGE_SIZE, get_frame_alloc_for_sure};

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>, app_list: boot::AppListRef) {
    // FIXME: set init process as Running
    init.write().resume();

    // FIXME: set processor's current pid to init's pid
    processor::set_pid(init.pid());

    PROCESS_MANAGER.call_once(|| ProcessManager::new(init, app_list));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

pub struct ProcessManager {
    processes: RwLock<HashMap<ProcessId, Arc<Process>, ahash::RandomState>>,
    ready_queue: Mutex<VecDeque<ProcessId>>,
    wait_queue: Mutex<HashMap<ProcessId, BTreeSet<ProcessId>, ahash::RandomState>>,
    app_list: boot::AppListRef,
}

impl ProcessManager {
    pub fn new(init: Arc<Process>, app_list: boot::AppListRef) -> Self {
        let mut processes = HashMap::default();
        let ready_queue = VecDeque::new();
        let wait_queue = HashMap::default();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
            wait_queue: Mutex::new(wait_queue),
            app_list,
        }
    }

    pub fn app_list(&self) -> boot::AppListRef {
        self.app_list
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    }

    #[inline]
    fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
    }

    #[inline]
    fn get_proc(&self, pid: &ProcessId) -> Option<Arc<Process>> {
        self.processes.read().get(pid).cloned()
    }

    pub fn current(&self) -> Arc<Process> {
        self.get_proc(&processor::get_pid())
            .expect("No current process")
    }

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        self.current().read().read(fd, buf)
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.current().read().write(fd, buf)
    }

    pub fn save_current(&self, context: &ProcessContext) {
        let current = self.current();
        let mut inner = current.write();
        // update current process's tick count
        if inner.status() != ProgramStatus::Dead {
            inner.tick();
        }
        // save current process's context
        inner.save(context);
    }

    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {
        loop {
            //fetch the next process from ready queue
            let pid = self
                .ready_queue
                .lock()
                .pop_front()
                .expect("No ready process to switch to.");

            let Some(next) = self.get_proc(&pid) else {
                continue;
            };

            let mut inner = next.write();
            // check if the next process is ready,
            // continue to fetch if not ready
            if !inner.is_ready() {
                continue;
            }
            // restore next process's context
            inner.restore(context);
            drop(inner);
            // update processor's current pid
            processor::set_pid(pid);

            // return next process's pid
            return pid;
        }
    }

    pub fn spawn_kernel_thread(
        &self,
        entry: VirtAddr,
        name: String,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap();
        let page_table = kproc.read().clone_page_table();
        let proc_vm = Some(ProcessVm::new(page_table));
        let proc = Process::new(name, Some(Arc::downgrade(&kproc)), proc_vm, proc_data);

        // alloc stack for the new process base on pid
        let stack_top = proc.alloc_init_stack();

        // FIXME: set the stack frame
        proc.write().init_stack_frame(entry, stack_top);
        // FIXME: add to process map
        let pid = proc.pid();
        info!("Process #{} stack top: {:#x}", pid, stack_top);
        self.add_proc(pid, proc);
        // FIXME: push to ready queue
        self.push_ready(pid);
        // FIXME: return new process pid
        pid
    }

    pub fn spawn(
        &self,
        elf: &xmas_elf::ElfFile,
        name: String,
        parent: Option<alloc::sync::Weak<Process>>,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap();
        let page_table = kproc.read().clone_page_table();
        let proc_vm = Some(ProcessVm::new(page_table));
        let proc = Process::new(name, parent, proc_vm, proc_data);

        let mut inner = proc.write();
        // load elf to process pagetable
        let code_pages = inner.load_elf(elf);
        inner.set_code_pages(code_pages);
        // alloc new stack for process
        let stack_top = inner.vm_mut().init_proc_stack(proc.pid());
        // mark process as ready
        inner.init_stack_frame(VirtAddr::new(elf.header.pt2.entry_point()), stack_top);
        inner.pause();
        drop(inner);

        trace!("New {:#?}", &proc);

        let pid = proc.pid();
        // something like kernel thread
        self.add_proc(pid, proc);
        self.push_ready(pid);

        pid
    }

    pub fn fork(&self) -> ProcessId {
        let parent = self.current();
        let child = parent.fork();
        let pid = child.pid();

        trace!("Forked process: parent #{} -> child #{}", parent.pid(), pid);
        self.add_proc(pid, child);

        pid
    }

    pub fn kill_current(&self, ret: isize) {
        self.kill(processor::get_pid(), ret);
    }

    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // FIXME: handle page fault
        if err_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            warn!(
                "Reject page fault caused by protection violation: addr={:#x}, err={:?}",
                addr, err_code
            );
            return false;
        }

        let current = self.current();
        let pid = current.pid();
        let handled = current.write().handle_page_fault(addr);

        if handled {
            info!("Handled page fault for process #{} at {:#x}", pid, addr);
        }

        handled
    }

    pub fn exit_code(&self, pid: ProcessId) -> Option<isize> {
        self.get_proc(&pid).and_then(|proc| proc.read().exit_code())
    }

    pub fn is_alive(&self, pid: ProcessId) -> bool {
        self.get_proc(&pid)
            .map(|proc| proc.read().status() != ProgramStatus::Dead)
            .unwrap_or(false)
    }

    pub fn wait_pid(&self, pid: ProcessId) {
        let waiter = processor::get_pid();
        self.wait_queue
            .lock()
            .entry(pid)
            .or_default()
            .insert(waiter);
    }

    pub fn block(&self, pid: ProcessId) {
        if let Some(proc) = self.get_proc(&pid) {
            proc.write().block();
        }
    }

    pub fn wake_up(&self, pid: ProcessId, ret: Option<isize>) {
        if let Some(proc) = self.get_proc(&pid) {
            let mut inner = proc.write();

            if let Some(ret) = ret {
                inner.set_return_value(ret);
            }

            inner.pause();
            drop(inner);

            self.push_ready(pid);
        }
    }

    pub fn kill(&self, pid: ProcessId, ret: isize) {
        let proc = self.get_proc(&pid);

        if proc.is_none() {
            warn!("Process #{} not found.", pid);
            return;
        }

        let proc = proc.unwrap();

        if proc.read().status() == ProgramStatus::Dead {
            warn!("Process #{} is already dead.", pid);
            return;
        }

        trace!("Kill {:#?}", &proc);

        proc.kill(ret);

        if let Some(waiters) = self.wait_queue.lock().remove(&pid) {
            for waiter in waiters {
                self.wake_up(waiter, Some(ret));
            }
        }
    }

    pub fn print_process_list(&self) {
        let mut output =
            String::from("  PID | PPID | Process Name | Pages |  Memory   |  Ticks  | Status\n");

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

        let alloc = get_frame_alloc_for_sure();
        let frames_used = alloc.frames_used();
        let frames_total = alloc.frames_total();
        let used = frames_used * PAGE_SIZE as usize;
        let total = frames_total * PAGE_SIZE as usize;
        output += &format_usage("Memory", used, total);
        drop(alloc);

        output += format!("Queue  : {:?}\n", self.ready_queue.lock()).as_str();

        output += &processor::print_processors();

        print!("{}", output);
    }
}

fn format_usage(name: &str, used: usize, total: usize) -> String {
    let (used_float, used_unit) = crate::humanized_size(used as u64);
    let (total_float, total_unit) = crate::humanized_size(total as u64);

    format!(
        "{:<6} : {:>6.2} {:>3} / {:>6.2} {:>3} ({:>5.2}%)\n",
        name,
        used_float,
        used_unit,
        total_float,
        total_unit,
        used as f32 / total as f32 * 100.0
    )
}
