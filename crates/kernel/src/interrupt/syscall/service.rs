use core::alloc::Layout;

use chrono::NaiveDate;
use storage::FileSystem;
use x86_64::VirtAddr;

use super::SyscallArgs;
use crate::{drivers::filesystem, proc, proc::*, utils::Resource};

pub fn spawn_process(args: &SyscallArgs) -> usize {
    // get app path by args
    // - core::str::from_utf8_unchecked
    // - core::slice::from_raw_parts
    // spawn the process by path
    // handle spawn error, return 0 if failed
    // return pid as usize
    let path = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            args.arg0 as *const u8,
            args.arg1,
        ))
    };

    proc::spawn(path)
        .map(|pid| u16::from(pid) as usize)
        .unwrap_or(0)
}

pub fn sys_write(args: &SyscallArgs) -> usize {
    // get buffer and fd by args
    // - core::slice::from_raw_parts
    // call proc::write -> isize
    // return the result as usize
    let fd = args.arg0 as u8;
    let buf = unsafe { core::slice::from_raw_parts(args.arg1 as *const u8, args.arg2) };
    proc::write(fd, buf) as usize
}

pub fn sys_read(args: &SyscallArgs) -> usize {
    // just like sys_write
    let fd = args.arg0 as u8;
    let buf = unsafe { core::slice::from_raw_parts_mut(args.arg1 as *mut u8, args.arg2) };
    proc::read(fd, buf) as usize
}

pub fn sys_open(args: &SyscallArgs) -> usize {
    let path = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            args.arg0 as *const u8,
            args.arg1,
        ))
    };

    match filesystem::get_rootfs().open_file(path) {
        Ok(file) => proc::open(Resource::File(file)) as usize,
        Err(err) => {
            warn!("failed to open {}: {:?}", path, err);
            (-1isize) as usize
        }
    }
}

pub fn sys_close(args: &SyscallArgs) -> usize {
    if proc::close(args.arg0 as u8) {
        0
    } else {
        (-1isize) as usize
    }
}

pub fn sys_list_dir(args: &SyscallArgs) -> usize {
    let path = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            args.arg0 as *const u8,
            args.arg1,
        ))
    };

    if filesystem::ls(path) {
        0
    } else {
        (-1isize) as usize
    }
}

pub fn sys_brk(args: &SyscallArgs) -> usize {
    let new_heap_end = if args.arg0 == 0 {
        None
    } else {
        Some(VirtAddr::new(args.arg0 as u64))
    };

    match proc::brk(new_heap_end) {
        Some(new_heap_end) => new_heap_end.as_u64() as usize,
        None => !0,
    }
}

pub fn exit_process(args: &SyscallArgs, context: &mut ProcessContext) {
    // exit process with retcode
    proc::exit(args.arg0 as isize, context);
}

pub fn list_process() {
    // list all processes
    proc::print_process_list();
}

pub fn new_sem(key: u32, value: usize) -> usize {
    let inserted = proc::get_process_manager()
        .current()
        .write()
        .sem_new(key, value);

    if inserted { 0 } else { 1 }
}

pub fn remove_sem(key: u32) -> usize {
    let removed = proc::get_process_manager()
        .current()
        .write()
        .sem_remove(key);

    if removed { 0 } else { 1 }
}

pub fn sem_wait(key: u32, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = proc::get_process_manager();
        let pid = proc::get_pid();
        let ret = manager.current().read().sem_wait(key, pid);

        match ret {
            SemaphoreResult::Ok => context.set_rax(0),
            SemaphoreResult::NotExist => context.set_rax(1),
            SemaphoreResult::Block(pid) => {
                context.set_rax(0);
                manager.save_current(context);
                manager.block(pid);
                manager.switch_next(context);
            }
            SemaphoreResult::WakeUp(_) => unreachable!(),
        }
    });
}

pub fn sem_signal(key: u32, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = proc::get_process_manager();
        let ret = manager.current().read().sem_signal(key);

        match ret {
            SemaphoreResult::Ok => context.set_rax(0),
            SemaphoreResult::NotExist => context.set_rax(1),
            SemaphoreResult::WakeUp(pid) => {
                manager.wake_up(pid, None);
                context.set_rax(0);
            }
            SemaphoreResult::Block(_) => unreachable!(),
        }
    });
}

pub fn sys_sem(args: &SyscallArgs, context: &mut ProcessContext) {
    match args.arg0 {
        0 => context.set_rax(new_sem(args.arg1 as u32, args.arg2)),
        1 => context.set_rax(remove_sem(args.arg1 as u32)),
        2 => sem_signal(args.arg1 as u32, context),
        3 => sem_wait(args.arg1 as u32, context),
        _ => context.set_rax(usize::MAX),
    }
}

pub fn sys_time() -> usize {
    let time = uefi::runtime::get_time().expect("Failed to get UEFI time");
    let naive = NaiveDate::from_ymd_opt(time.year().into(), time.month().into(), time.day().into())
        .expect("Invalid UEFI date")
        .and_hms_nano_opt(
            time.hour().into(),
            time.minute().into(),
            time.second().into(),
            time.nanosecond(),
        )
        .expect("Invalid UEFI time");

    naive.and_utc().timestamp_millis() as usize
}

pub fn sys_allocate(args: &SyscallArgs) -> usize {
    let layout = unsafe { (args.arg0 as *const Layout).as_ref().unwrap() };

    if layout.size() == 0 {
        return 0;
    }

    let ret = crate::memory::user::USER_ALLOCATOR
        .lock()
        .allocate_first_fit(*layout);

    match ret {
        Ok(ptr) => ptr.as_ptr() as usize,
        Err(_) => 0,
    }
}

pub fn sys_deallocate(args: &SyscallArgs) {
    let layout = unsafe { (args.arg1 as *const Layout).as_ref().unwrap() };

    if args.arg0 == 0 || layout.size() == 0 {
        return;
    }

    let ptr = args.arg0 as *mut u8;

    unsafe {
        crate::memory::user::USER_ALLOCATOR
            .lock()
            .deallocate(core::ptr::NonNull::new_unchecked(ptr), *layout);
    }
}
