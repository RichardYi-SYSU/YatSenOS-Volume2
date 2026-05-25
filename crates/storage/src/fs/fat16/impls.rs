use alloc::{boxed::Box, string::String, vec::Vec};

use super::*;

impl Fat16Impl {
    pub fn new(inner: impl BlockDevice<Block512>) -> Self {
        let mut block = Block::default();
        let block_size = Block512::size();

        inner.read_block(0, &mut block).unwrap();
        let bpb = Fat16Bpb::new(block.as_ref()).unwrap();

        trace!("Loading Fat16 Volume: {:#?}", bpb);

        // HINT: FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz) +
        // RootDirSectors;
        let fat_start = bpb.reserved_sector_count() as usize;
        let root_dir_size =
            (bpb.root_entries_count() as usize * DirEntry::LEN).div_ceil(block_size);
        let first_root_dir_sector =
            fat_start + bpb.fat_count() as usize * bpb.sectors_per_fat() as usize;
        let first_data_sector = first_root_dir_sector + root_dir_size;

        Self {
            bpb,
            inner: Box::new(inner),
            fat_start,
            first_data_sector,
            first_root_dir_sector,
        }
    }

    pub fn cluster_to_sector(&self, cluster: &Cluster) -> usize {
        match *cluster {
            Cluster::ROOT_DIR => self.first_root_dir_sector,
            Cluster(c) => {
                ((c as usize - 2) * self.bpb.sectors_per_cluster() as usize)
                    + self.first_data_sector
            }
        }
    }

    fn root_dir_sector_count(&self) -> usize {
        (self.bpb.root_entries_count() as usize * DirEntry::LEN).div_ceil(BLOCK_SIZE)
    }

    pub(crate) fn cluster_size(&self) -> usize {
        self.bpb.sectors_per_cluster() as usize * BLOCK_SIZE
    }

    pub(crate) fn read_sector(&self, sector: usize, block: &mut Block512) -> FsResult {
        self.inner.read_block(sector, block)
    }

    pub(crate) fn next_cluster(&self, cluster: Cluster) -> FsResult<Cluster> {
        let fat_offset = cluster.0 as usize * 2;
        let fat_sector = self.fat_start + fat_offset / BLOCK_SIZE;
        let entry_offset = fat_offset % BLOCK_SIZE;
        let mut block = Block512::default();

        self.read_sector(fat_sector, &mut block)?;
        let next = u16::from_le_bytes(block[entry_offset..entry_offset + 2].try_into().unwrap());

        match next {
            0x0000 => Ok(Cluster::EMPTY),
            0x0001 => Ok(Cluster::INVALID),
            0x0002..=0xFFF6 => Ok(Cluster(next as u32)),
            0xFFF7 => Ok(Cluster::BAD),
            _ => Ok(Cluster::END_OF_FILE),
        }
    }

    pub(crate) fn cluster_for_offset(&self, start: Cluster, offset: usize) -> FsResult<Cluster> {
        let mut cluster = start;
        for _ in 0..offset / self.cluster_size() {
            cluster = self.next_cluster(cluster)?;
            match cluster {
                Cluster::END_OF_FILE => return Err(FsError::EndOfFile),
                Cluster::BAD => return Err(FsError::BadCluster),
                Cluster::EMPTY | Cluster::INVALID => return Err(FsError::InvalidOperation),
                _ => {}
            }
        }
        Ok(cluster)
    }

    fn read_directory(&self, dir: &Directory) -> FsResult<Vec<DirEntry>> {
        let mut entries = Vec::new();

        if dir.cluster == Cluster::ROOT_DIR {
            self.read_dir_sectors(
                self.first_root_dir_sector,
                self.root_dir_sector_count(),
                &mut entries,
            )?;
        } else {
            let mut cluster = dir.cluster;
            loop {
                let first_sector = self.cluster_to_sector(&cluster);
                self.read_dir_sectors(
                    first_sector,
                    self.bpb.sectors_per_cluster() as usize,
                    &mut entries,
                )?;

                cluster = self.next_cluster(cluster)?;
                match cluster {
                    Cluster::END_OF_FILE => break,
                    Cluster::BAD => return Err(FsError::BadCluster),
                    Cluster::EMPTY | Cluster::INVALID => return Err(FsError::InvalidOperation),
                    _ => {}
                }
            }
        }

        Ok(entries)
    }

    fn read_dir_sectors(
        &self,
        first_sector: usize,
        sectors: usize,
        entries: &mut Vec<DirEntry>,
    ) -> FsResult {
        let mut block = Block512::default();

        for sector in first_sector..first_sector + sectors {
            self.read_sector(sector, &mut block)?;
            for raw in block.as_ref().chunks_exact(DirEntry::LEN) {
                let filename = ShortFileName::new(&raw[..11]);
                if filename.is_eod() {
                    return Ok(());
                }
                if filename.is_unused() {
                    continue;
                }

                let entry = DirEntry::parse(raw)?;
                if entry.is_valid()
                    && !entry.is_long_name()
                    && !entry.attributes.contains(Attributes::VOLUME_ID)
                {
                    entries.push(entry);
                }
            }
        }

        Ok(())
    }

    fn open_dir(&self, path: &str) -> FsResult<Directory> {
        let mut dir = Directory::root();

        for component in Self::path_components(path) {
            let filename = ShortFileName::parse(component)?;
            let entry = self
                .read_directory(&dir)?
                .into_iter()
                .find(|entry| entry.filename.matches(&filename))
                .ok_or(FsError::FileNotFound)?;

            if !entry.is_directory() {
                return Err(FsError::NotADirectory);
            }
            dir = Directory::from_entry(entry);
        }

        Ok(dir)
    }

    fn find_entry(&self, path: &str) -> FsResult<Option<DirEntry>> {
        let mut components: Vec<&str> = Self::path_components(path).collect();
        let Some(last) = components.pop() else {
            return Ok(None);
        };

        let parent_path = components.join("/");
        let parent = self.open_dir(&parent_path)?;
        let filename = ShortFileName::parse(last)?;

        Ok(self
            .read_directory(&parent)?
            .into_iter()
            .find(|entry| entry.filename.matches(&filename)))
    }

    fn path_components(path: &str) -> impl Iterator<Item = &str> {
        path.split(PATH_SEPARATOR).filter(|part| !part.is_empty())
    }
}

impl FileSystem for Fat16 {
    fn read_dir(&self, path: &str) -> FsResult<Box<dyn Iterator<Item = Metadata> + Send>> {
        let dir = self.handle.open_dir(path)?;
        let entries = self.handle.read_directory(&dir)?;
        Ok(Box::new(
            entries
                .into_iter()
                .map(|entry| entry.as_meta())
                .collect::<Vec<_>>()
                .into_iter(),
        ))
    }

    fn open_file(&self, path: &str) -> FsResult<FileHandle> {
        let entry = self.handle.find_entry(path)?.ok_or(FsError::FileNotFound)?;
        if !entry.is_file() {
            return Err(FsError::NotAFile);
        }

        Ok(FileHandle::new(
            entry.as_meta(),
            Box::new(File::new(self.handle.clone(), entry)),
        ))
    }

    fn metadata(&self, path: &str) -> FsResult<Metadata> {
        if Self::handle_path_is_root(path) {
            return Ok(Metadata::new(
                String::from("/"),
                FileType::Directory,
                0,
                None,
                None,
                None,
            ));
        }

        self.handle
            .find_entry(path)?
            .map(|entry| entry.as_meta())
            .ok_or(FsError::FileNotFound)
    }

    fn exists(&self, path: &str) -> FsResult<bool> {
        Ok(self.metadata(path).is_ok())
    }
}

impl Fat16 {
    fn handle_path_is_root(path: &str) -> bool {
        Fat16Impl::path_components(path).next().is_none()
    }
}
