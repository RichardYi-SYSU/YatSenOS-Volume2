use core::alloc::Layout;

use chrono::NaiveDate;
use super::SyscallArgs;
use crate::{proc, proc::*};

pub fn spawn_process(args: &SyscallArgs) -> usize {
    // get app name by args
    // - core::str::from_utf8_unchecked
    // - core::slice::from_raw_parts
    // spawn the process by name
    // handle spawn error, return 0 if failed
    // return pid as usize
    let name = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            args.arg0 as *const u8,
            args.arg1,
        ))
    };

    proc::spawn(name)
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

pub fn exit_process(args: &SyscallArgs, context: &mut ProcessContext) {
    // exit process with retcode
    proc::exit(args.arg0 as isize, context);
}

pub fn list_process() {
    // list all processes
    proc::print_process_list();
}

pub fn sys_time() -> usize {
    let time = uefi::runtime::get_time().expect("Failed to get UEFI time");
    let naive = NaiveDate::from_ymd_opt(
        time.year().into(),
        time.month().into(),
        time.day().into(),
    )
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
