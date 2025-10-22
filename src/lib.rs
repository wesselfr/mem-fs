#![cfg_attr(not(feature = "std"), no_std)]

use core::str::FromStr;

use heapless::{String, Vec};

const MAX_FILE_NAME_LENGHT: usize = 255;
const MAX_NUM_FILES: usize = 32;
const STORAGE_SIZE: usize = 4096;

pub struct FileEntry {
    pub name: String<MAX_FILE_NAME_LENGHT>,
    pub offset: usize,
    pub size: usize,
}

pub struct MemoryFs {
    pub entries: Vec<FileEntry, MAX_NUM_FILES>,
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

    // File system operations
    // TODO: Implement a filesystem trait for these functions
    // TODO: Support atomic operations
    pub fn create(&mut self, name: &str, data: &[u8]) -> Result<(), &'static str> {
        if self.used + data.len() > STORAGE_SIZE {
            return Err("Not enough space");
        }

        // Check if we have space for another entry
        // FIXME: FileEntry should not be a limiting factor for adding files, storage space should be the only limit.
        if name.len() > MAX_FILE_NAME_LENGHT {
            return Err("Filename is too big");
        }

        let offset = self.used;
        self.entries
            .push(FileEntry {
                name: String::from_str(name).expect("Error while processing the filename"),
                offset,
                size: data.len(),
            })
            .map_err(|_| "Too many files")?;

        // Insert the data into the file system.
        self.storage[offset..offset + data.len()].copy_from_slice(data);
        self.used += data.len();

        Ok(())
    }
    pub fn read(&self, name: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|f| f.name == name)
            .map(|f| &self.storage[f.offset..f.offset + f.size])
    }
    pub fn delete(&mut self, name: &str) -> Result<(), &'static str> {
        todo!()
    }

    // Debug
    pub fn list_files(&self) {
        println!("File entries:");
        for entry in &self.entries {
            println!("\t{} ({} bytes @ {})", entry.name, entry.size, entry.offset);
        }
    }

    /// Visualize the filesystem in hex format.
    pub fn hex_dump(&self) {
        todo!()
    }
}
