//! Directory Entry
//!
//! reference: <https://wiki.osdev.org/FAT#Directories_on_FAT12.2F16.2F32>

use core::{
    fmt::{Debug, Display},
    ops::*,
};

use bitflags::bitflags;
use chrono::{DateTime, LocalResult::Single, TimeZone, Utc};

use crate::*;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DirEntry {
    pub filename: ShortFileName,
    pub modified_time: FsTime,
    pub created_time: FsTime,
    pub accessed_time: FsTime,
    pub cluster: Cluster,
    pub attributes: Attributes,
    pub size: u32,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Cluster(pub u32);

bitflags! {
    /// File Attributes
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Attributes: u8 {
        const READ_ONLY = 0x01;
        const HIDDEN    = 0x02;
        const SYSTEM    = 0x04;
        const VOLUME_ID = 0x08;
        const DIRECTORY = 0x10;
        const ARCHIVE   = 0x20;
        const LFN       = 0x0f; // Long File Name, Not Implemented
    }
}

impl DirEntry {
    pub const LEN: usize = 0x20;

    pub fn filename(&self) -> String {
        // NOTE: ignore the long file name in FAT16 for lab
        if self.is_valid() && !self.is_long_name() {
            format!("{}", self.filename)
        } else {
            String::from("unknown")
        }
    }

    /// For Standard 8.3 format
    ///
    /// reference: https://osdev.org/FAT#Standard_8.3_format
    pub fn parse(data: &[u8]) -> FsResult<DirEntry> {
        if data.len() < Self::LEN {
            return Err(FilenameError::UnableToParse.into());
        }

        let filename = ShortFileName::new(&data[..11]);
        let attributes = Attributes::from_bits_truncate(data[11]);
        let created_time = parse_datetime(
            ((u16::from_le_bytes(data[16..18].try_into().unwrap()) as u32) << 16)
                | u16::from_le_bytes(data[14..16].try_into().unwrap()) as u32,
        );
        let accessed_time =
            parse_datetime((u16::from_le_bytes(data[18..20].try_into().unwrap()) as u32) << 16);
        let modified_time = parse_datetime(
            ((u16::from_le_bytes(data[24..26].try_into().unwrap()) as u32) << 16)
                | u16::from_le_bytes(data[22..24].try_into().unwrap()) as u32,
        );
        let cluster_hi = u16::from_le_bytes(data[20..22].try_into().unwrap()) as u32;
        let cluster_lo = u16::from_le_bytes(data[26..28].try_into().unwrap()) as u32;
        let cluster = (cluster_hi << 16) | cluster_lo;
        let size = u32::from_le_bytes(data[28..32].try_into().unwrap());

        Ok(DirEntry {
            filename,
            modified_time,
            created_time,
            accessed_time,
            cluster: Cluster(cluster),
            attributes,
            size,
        })
    }

    pub fn as_meta(&self) -> Metadata {
        self.into()
    }

    pub fn is_valid(&self) -> bool {
        !self.filename.is_eod() && !self.filename.is_unused()
    }

    pub fn is_long_name(&self) -> bool {
        self.attributes.bits() == Attributes::LFN.bits()
    }

    pub fn is_directory(&self) -> bool {
        self.attributes.contains(Attributes::DIRECTORY)
    }

    pub fn is_file(&self) -> bool {
        self.is_valid()
            && !self.is_directory()
            && !self.is_long_name()
            && !self.attributes.contains(Attributes::VOLUME_ID)
    }
}

fn parse_datetime(time: u32) -> FsTime {
    let raw_time = (time & 0xFFFF) as u16;
    let raw_date = (time >> 16) as u16;

    let sec = ((raw_time & 0x1F) * 2) as u32;
    let min = ((raw_time >> 5) & 0x3F) as u32;
    let hour = ((raw_time >> 11) & 0x1F) as u32;
    let day = (raw_date & 0x1F) as u32;
    let month = ((raw_date >> 5) & 0x0F) as u32;
    let year = 1980 + ((raw_date >> 9) & 0x7F) as i32;

    if let Single(time) = Utc.with_ymd_and_hms(year, month, day, hour, min, sec) {
        time
    } else {
        DateTime::from_timestamp_millis(0).unwrap()
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct ShortFileName {
    pub name: [u8; 8],
    pub ext: [u8; 3],
}

impl ShortFileName {
    pub fn new(buf: &[u8]) -> Self {
        Self {
            name: buf[..8].try_into().unwrap(),
            ext: buf[8..11].try_into().unwrap(),
        }
    }

    pub fn basename(&self) -> &str {
        core::str::from_utf8(&self.name).unwrap()
    }

    pub fn extension(&self) -> &str {
        core::str::from_utf8(&self.ext).unwrap()
    }

    pub fn is_eod(&self) -> bool {
        self.name[0] == 0x00 && self.ext[0] == 0x00
    }

    pub fn is_unused(&self) -> bool {
        self.name[0] == 0xE5
    }

    pub fn matches(&self, sfn: &ShortFileName) -> bool {
        self.name == sfn.name && self.ext == sfn.ext
    }

    /// Parse a short file name from a string
    pub fn parse(name: &str) -> FsResult<ShortFileName> {
        if name.is_empty() {
            return Err(FilenameError::FilenameEmpty.into());
        }

        let mut base = [0x20u8; 8];
        let mut ext = [0x20u8; 3];
        let mut in_ext = false;
        let mut base_len = 0usize;
        let mut ext_len = 0usize;

        for byte in name.bytes() {
            if byte == b'.' {
                if in_ext || base_len == 0 || base_len > 8 {
                    return Err(FilenameError::MisplacedPeriod.into());
                }
                in_ext = true;
                continue;
            }

            if is_invalid_sfn_char(byte) {
                return Err(FilenameError::InvalidCharacter.into());
            }

            let byte = byte.to_ascii_uppercase();
            if in_ext {
                if ext_len >= ext.len() {
                    return Err(FilenameError::NameTooLong.into());
                }
                ext[ext_len] = byte;
                ext_len += 1;
            } else {
                if base_len >= base.len() {
                    return Err(FilenameError::NameTooLong.into());
                }
                base[base_len] = byte;
                base_len += 1;
            }
        }

        if base_len == 0 {
            return Err(FilenameError::FilenameEmpty.into());
        }

        Ok(ShortFileName { name: base, ext })
    }
}

fn is_invalid_sfn_char(byte: u8) -> bool {
    matches!(
        byte,
        0x00..=0x20
            | 0x22
            | 0x2A
            | 0x2B
            | 0x2C
            | 0x2F
            | 0x3A
            | 0x3B
            | 0x3C
            | 0x3D
            | 0x3E
            | 0x3F
            | 0x5B
            | 0x5C
            | 0x5D
            | 0x7C
    )
}

impl Debug for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Display for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.ext[0] == 0x20 {
            write!(f, "{}", self.basename().trim_end())
        } else {
            write!(
                f,
                "{}.{}",
                self.basename().trim_end(),
                self.extension().trim_end()
            )
        }
    }
}

impl Cluster {
    /// Magic value indicating an invalid cluster value.
    pub const INVALID: Cluster = Cluster(0xFFFF_FFF6);
    /// Magic value indicating a bad cluster.
    pub const BAD: Cluster = Cluster(0xFFFF_FFF7);
    /// Magic value indicating a empty cluster.
    pub const EMPTY: Cluster = Cluster(0x0000_0000);
    /// Magic value indicating the cluster holding the root directory
    /// (which doesn't have a number in Fat16 as there's a reserved region).
    pub const ROOT_DIR: Cluster = Cluster(0xFFFF_FFFC);
    /// Magic value indicating that the cluster is allocated and is the final
    /// cluster for the file
    pub const END_OF_FILE: Cluster = Cluster(0xFFFF_FFFF);
}

impl Add<u32> for Cluster {
    type Output = Cluster;
    fn add(self, rhs: u32) -> Cluster {
        Cluster(self.0 + rhs)
    }
}

impl AddAssign<u32> for Cluster {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

impl Add<Cluster> for Cluster {
    type Output = Cluster;
    fn add(self, rhs: Cluster) -> Cluster {
        Cluster(self.0 + rhs.0)
    }
}

impl AddAssign<Cluster> for Cluster {
    fn add_assign(&mut self, rhs: Cluster) {
        self.0 += rhs.0;
    }
}

impl From<&DirEntry> for Metadata {
    fn from(entry: &DirEntry) -> Metadata {
        Metadata {
            entry_type: if entry.is_directory() {
                FileType::Directory
            } else {
                FileType::File
            },
            name: entry.filename(),
            len: entry.size as usize,
            created: Some(entry.created_time),
            accessed: Some(entry.accessed_time),
            modified: Some(entry.modified_time),
        }
    }
}

impl Display for Cluster {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

impl Debug for Cluster {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_entry() {
        let data = hex_literal::hex!(
            "4b 45 52 4e 45 4c 20 20 45 4c 46 20 00 00 0f be
             d0 50 d0 50 00 00 0f be d0 50 02 00 f0 e4 0e 00"
        );

        let res = DirEntry::parse(&data).unwrap();

        assert_eq!(&res.filename.name, b"KERNEL  ");
        assert_eq!(&res.filename.ext, b"ELF");
        assert_eq!(res.attributes, Attributes::ARCHIVE);
        assert_eq!(res.cluster, Cluster(2));
        assert_eq!(res.size, 0xee4f0);
        assert_eq!(
            res.created_time,
            Utc.with_ymd_and_hms(2020, 6, 16, 23, 48, 30).unwrap()
        );
        assert_eq!(
            res.modified_time,
            Utc.with_ymd_and_hms(2020, 6, 16, 23, 48, 30).unwrap()
        );
        assert_eq!(
            res.accessed_time,
            Utc.with_ymd_and_hms(2020, 6, 16, 0, 0, 0).unwrap()
        );

        println!("{:#?}", res);
    }
}
