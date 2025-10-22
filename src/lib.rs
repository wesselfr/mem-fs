#![cfg_attr(not(feature = "std"), no_std)]

use heapless::{String, Vec};

const MAX_FILE_NAME_LENGHT: usize = 255;
const STORAGE_ENTRIES: usize = 4096;
const STORAGE_SIZE: usize = 4096;

pub struct FileEntry {
    pub name: String<MAX_FILE_NAME_LENGHT>,
    pub offset: usize,
    pub size: usize,
}

pub struct MemoryFs {
    pub entries: Vec<FileEntry, STORAGE_ENTRIES>,
    pub storage: [u8; STORAGE_SIZE],
    pub used: usize,
}

impl MemoryFs {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            storage: [0; STORAGE_SIZE],
            used: 0,
        }
    }

    pub fn create(&mut self, name: &str, data: &[u8]) -> Result<(), &'static str> {
        if self.used + data.len() > STORAGE_SIZE {
            return Err("Not enough space");
        }
        let offset = self.used;
        self.storage[offset..offset + data.len()].copy_from_slice(data);
        self.used += data.len();

        let mut n = String::new();
        n.push_str(name).map_err(|_| "Name too long")?;
        self.entries
            .push(FileEntry {
                name: n,
                offset,
                size: data.len(),
            })
            .map_err(|_| "Too many files")?;

        Ok(())
    }

    pub fn read(&self, name: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|f| f.name == name)
            .map(|f| &self.storage[f.offset..f.offset + f.size])
    }
}
