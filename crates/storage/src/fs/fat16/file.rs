//! File
//!
//! reference: <https://wiki.osdev.org/FAT#Directories_on_FAT12.2F16.2F32>

use super::*;

#[derive(Debug, Clone)]
pub struct File {
    /// The current offset in the file
    offset: usize,
    /// The current cluster of this file
    current_cluster: Cluster,
    /// DirEntry of this file
    entry: DirEntry,
    /// The file system handle that contains this file
    handle: Fat16Handle,
}

impl File {
    pub fn new(handle: Fat16Handle, entry: DirEntry) -> Self {
        Self {
            offset: 0,
            current_cluster: entry.cluster,
            entry,
            handle,
        }
    }

    pub fn length(&self) -> usize {
        self.entry.size as usize
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.offset >= self.length() || buf.is_empty() {
            return Ok(0);
        }

        let readable = core::cmp::min(buf.len(), self.length() - self.offset);
        let cluster_size = self.handle.cluster_size();
        let mut copied = 0usize;
        let mut block = Block512::default();

        self.current_cluster = self
            .handle
            .cluster_for_offset(self.entry.cluster, self.offset)?;

        while copied < readable {
            let cluster_offset = self.offset % cluster_size;
            let sector_offset = cluster_offset / BLOCK_SIZE;
            let byte_offset = cluster_offset % BLOCK_SIZE;
            let sector = self.handle.cluster_to_sector(&self.current_cluster) + sector_offset;

            self.handle.read_sector(sector, &mut block)?;

            let available_in_sector = BLOCK_SIZE - byte_offset;
            let chunk_len = core::cmp::min(available_in_sector, readable - copied);
            buf[copied..copied + chunk_len]
                .copy_from_slice(&block[byte_offset..byte_offset + chunk_len]);

            copied += chunk_len;
            self.offset += chunk_len;

            if self.offset < self.length() && self.offset % cluster_size == 0 {
                self.current_cluster = self.handle.next_cluster(self.current_cluster)?;
                match self.current_cluster {
                    Cluster::END_OF_FILE => break,
                    Cluster::BAD => return Err(FsError::BadCluster),
                    Cluster::EMPTY | Cluster::INVALID => return Err(FsError::InvalidOperation),
                    _ => {}
                }
            }
        }

        Ok(copied)
    }
}

// NOTE: `Seek` trait is not required for this lab
impl Seek for File {
    fn seek(&mut self, _pos: SeekFrom) -> FsResult<usize> {
        unimplemented!()
    }
}

// NOTE: `Write` trait is not required for this lab
impl Write for File {
    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        unimplemented!()
    }

    fn flush(&mut self) -> FsResult {
        unimplemented!()
    }
}
