use alloc::{boxed::Box, format};

use chrono::{Datelike, Timelike};
use storage::{fat16::Fat16, mbr::*, *};

use super::ata::*;

pub static ROOTFS: spin::Once<Mount> = spin::Once::new();

pub fn get_rootfs() -> &'static Mount {
    ROOTFS.get().unwrap()
}

pub fn init() {
    info!("Opening disk device...");

    let drive = AtaDrive::open(0, 0).expect("Failed to open disk device");

    // only get the first partition
    let part = MbrTable::parse(drive)
        .expect("Failed to parse MBR")
        .partitions()
        .expect("Failed to get partitions")
        .remove(0);

    info!("Mounting filesystem...");

    ROOTFS.call_once(|| Mount::new(Box::new(Fat16::new(part)), "/".into()));

    trace!("Root filesystem: {:#?}", ROOTFS.get().unwrap());

    info!("Initialized Filesystem.");
}

pub fn ls(root_path: &str) -> bool {
    let iter = match get_rootfs().read_dir(root_path) {
        Ok(iter) => iter,
        Err(err) => {
            warn!("{:?}", err);
            return false;
        }
    };

    println!("{:<24} {:>10} {:<4} {}", "Name", "Size", "Type", "Modified");
    println!("{:-<24} {:-<10} {:-<4} {:-<19}", "", "", "", "");

    for meta in iter {
        let is_dir = meta.is_dir();
        let name = if is_dir {
            format!("{}/", meta.name)
        } else {
            meta.name
        };
        let kind = if is_dir { "dir" } else { "file" };
        let (size, unit) = crate::humanized_size_short(meta.len as u64);

        if let Some(time) = meta.modified {
            println!(
                "{:<24} {:>6.1} {:<3} {:<4} {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                name,
                size,
                unit,
                kind,
                time.year(),
                time.month(),
                time.day(),
                time.hour(),
                time.minute(),
                time.second()
            );
        } else {
            println!(
                "{:<24} {:>6.1} {:<3} {:<4} -",
                name, size, unit, kind
            );
        }
    }

    true
}
