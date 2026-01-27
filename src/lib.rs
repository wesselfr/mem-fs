#![cfg_attr(not(feature = "std"), no_std)]

use core::str::FromStr;
use heapless::{String, Vec};

const MAX_FILE_NAME_LENGTH: usize = 255;
const MAX_NUM_FILES: usize = 32;

pub const DEFAULT_STORAGE_SIZE: usize = 4096;
pub const DEFAULT_PAGE_SIZE: usize = 32;

const MAX_PAGE_BITMAP_WORDS: usize = 256;

#[derive(Debug)]
pub enum FsErr {
    ReadOnly,
    WouldFragment,
    TooManyExtents,
    NoSpace,
    NotFound,
    Duplicate,
    FileNameInvalid(&'static str),
    TooManyFiles, // TODO: Depricate when possible. Too many files should not be a limiting factor (only OutOfSpace).
    InvalidOp,
    Corrupt,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct FileFlags: u32{
        const IMMUTABLE=1<<0; // reject write/append/delete
        const DO_NOT_FRAGMENT=1<<1; // append must stay contiguous or fail
        const CHECKSUMMED=1<<2; // verify on read (unimplemented)
        const APPEND_ONLY=1<<3; // no write_at (unimplemented)
        const SEALED_NAMES=1<<4; // no rename allowed
    }
}

#[derive(Copy, Clone)]
struct Extent {
    // TODO: Consider u16 / u32 for start_page and len_page.
    start_page: usize,
    len_pages: usize,
}

pub struct FileEntry {
    pub name: String<MAX_FILE_NAME_LENGTH>,
    pub size: usize,
    flags: FileFlags,
    extent: Extent,
}

pub type MemFs = MemoryFs<DEFAULT_STORAGE_SIZE, DEFAULT_PAGE_SIZE>;
pub struct MemoryFs<const STORAGE_SIZE: usize, const PAGE_SIZE: usize> {
    entries: Vec<FileEntry, MAX_NUM_FILES>,
    storage: [u8; STORAGE_SIZE],
    page_bitmap: heapless::Vec<u32, MAX_PAGE_BITMAP_WORDS>,
}

impl<const STORAGE_SIZE: usize, const PAGE_SIZE: usize> Default
    for MemoryFs<STORAGE_SIZE, PAGE_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const STORAGE_SIZE: usize, const PAGE_SIZE: usize> MemoryFs<STORAGE_SIZE, PAGE_SIZE> {
    const fn num_pages() -> usize {
        STORAGE_SIZE / PAGE_SIZE
    }

    const fn bitmap_words() -> usize {
        Self::num_pages().div_ceil(32)
    }

    pub fn new() -> Self {
        assert!(PAGE_SIZE > 0);
        assert!(STORAGE_SIZE.is_multiple_of(PAGE_SIZE));

        let mut page_bitmap: heapless::Vec<u32, MAX_PAGE_BITMAP_WORDS> = heapless::Vec::new();
        let words = Self::bitmap_words();
        assert!(words <= MAX_PAGE_BITMAP_WORDS);

        for _ in 0..words {
            page_bitmap.push(0).ok();
        }

        Self {
            entries: Vec::new(),
            storage: [0; STORAGE_SIZE],
            page_bitmap,
        }
    }

    // File system operations
    // TODO: Implement a filesystem trait for these functions
    // TODO: Support atomic operations
    pub fn create(&mut self, name: &str, data: &[u8]) -> Result<(), FsErr> {
        self.create_with_flags(name, data, FileFlags::empty())
    }
    pub fn create_with_flags(
        &mut self,
        name: &str,
        data: &[u8],
        flags: FileFlags,
    ) -> Result<(), FsErr> {
        // Check if we have space for another entry
        if name.len() > MAX_FILE_NAME_LENGTH {
            return Err(FsErr::FileNameInvalid("File name too long"));
        }

        let required_pages = data.len().div_ceil(PAGE_SIZE);
        let extent = self.find_free_pages(required_pages);

        if extent.is_none() {
            return Err(FsErr::TooManyExtents);
        };
        let extent = extent.unwrap();

        let file_name: String<MAX_FILE_NAME_LENGTH> =
            String::from_str(name).expect("Error while processing filename");

        // Check for invalid or duplicate names.
        let file_name = self.validate_file_name(file_name)?;

        self.entries
            .push(FileEntry {
                name: file_name,
                size: data.len(),
                flags,
                extent,
            })
            // FIXME: FileEntry should not be a limiting factor for adding files, storage space should be the only limit.
            .map_err(|_| FsErr::TooManyFiles)?;

        self.mark_pages(extent.start_page, extent.len_pages, true);

        let offset = extent.start_page * PAGE_SIZE;
        self.storage[offset..offset + data.len()].copy_from_slice(data);

        Ok(())
    }
    pub fn read(&self, name: &str) -> Option<&[u8]> {
        self.entries.iter().find(|f| f.name == name).map(|f| {
            &self.storage[f.extent.start_page * PAGE_SIZE..f.extent.start_page * PAGE_SIZE + f.size]
        })
    }
    pub fn exists(&self, name: &str) -> bool {
        self.entries.iter().any(|f| f.name == name)
    }
    pub fn rename(&mut self, name: &str, new_name: &str) -> Result<(), FsErr> {
        let index = self.find_file_index(name)?;
        let new_name = self.validate_file_name(
            String::from_str(new_name)
                .map_err(|_| FsErr::FileNameInvalid("Error while processing file name"))?,
        )?;

        self.entries[index].name = new_name;
        Ok(())
    }
    /// Append data to file.
    ///
    ///
    pub fn append(&mut self, name: &str, data: &[u8]) -> Result<(), FsErr> {
        self.append_impl(name, data, true)
    }
    /// Append data to file.
    ///
    /// This function forces the data to be contiguous in memory.
    ///
    /// Use append_strict_or_repack if allowed to move data around to keep memory contiguous
    pub fn append_strict(&mut self, name: &str, data: &[u8]) -> Result<(), FsErr> {
        self.append_impl(name, data, false)
    }
    /// Similiar to append_strict, but allows data to be moved/repacked to ensure memory stays contiguous
    pub fn append_strict_or_repack(&mut self, name: &str, data: &[u8]) -> Result<(), FsErr> {
        self.append_impl(name, data, true)
    }
    fn append_impl(&mut self, name: &str, data: &[u8], repack: bool) -> Result<(), FsErr> {
        // No Op
        if data.is_empty() {
            return Ok(());
        }

        // Find file and check flags.
        let index = self.find_file_index(name)?;
        let entry = &self.entries[index];

        if entry.flags.contains(FileFlags::IMMUTABLE) {
            return Err(FsErr::ReadOnly);
        }

        // Current allocation and required space.
        let current_extent = self.entries[index].extent;
        let current_capacity = current_extent.len_pages * PAGE_SIZE;
        let required_size = self.entries[index].size + data.len();

        // Case 1: Fits current allocation.
        if required_size <= current_capacity {
            let offset = (current_extent.start_page * PAGE_SIZE) + self.entries[index].size;
            self.storage[offset..offset + data.len()].copy_from_slice(data);
            self.entries[index].size += data.len();

            return Ok(());
        }

        let required_pages = required_size.div_ceil(PAGE_SIZE);
        let extra_pages = required_pages.saturating_sub(current_extent.len_pages);

        assert!(extra_pages > 0);

        // Case 2: Can we extent the page?
        let next_page = current_extent.start_page + current_extent.len_pages;
        if let Some(neighbour) = self.check_neighbour_pages_free(next_page, extra_pages) {
            self.mark_pages(neighbour.start_page, neighbour.len_pages, true);
            self.entries[index].extent = Extent {
                start_page: current_extent.start_page,
                len_pages: current_extent.len_pages + neighbour.len_pages,
            };

            let offset = (current_extent.start_page * PAGE_SIZE) + self.entries[index].size;
            self.storage[offset..offset + data.len()].copy_from_slice(data);
            self.entries[index].size = required_size;
            return Ok(());
        };

        // Case 3: Relocate (repack) if allowed.
        if repack && let Some(new_extent) = self.find_free_pages(required_pages) {
            let old_start = current_extent.start_page * PAGE_SIZE;
            let old_len = self.entries[index].size;
            let old_range = old_start..old_start + old_len;

            let new_start = new_extent.start_page * PAGE_SIZE;

            self.mark_pages(new_extent.start_page, new_extent.len_pages, true);

            // Move exsisting bytes
            if old_len > 0 && old_start != new_start {
                self.storage.copy_within(old_range, new_start);
            }

            // Append new bytes.
            let append_offset = new_start + old_len;
            self.storage[append_offset..append_offset + data.len()].copy_from_slice(data);

            self.mark_pages(current_extent.start_page, current_extent.len_pages, false);

            self.entries[index].extent = new_extent;
            self.entries[index].size = required_size;
            return Ok(());
        }
        // Can't extend and repack is not allowed.
        Err(FsErr::WouldFragment)
    }
    pub fn delete(&mut self, name: &str) -> Result<(), FsErr> {
        let index = self.find_file_index(name)?;
        if self.entries[index].flags.contains(FileFlags::IMMUTABLE) {
            return Err(FsErr::ReadOnly);
        };
        let page_extent = self.entries[index].extent;

        self.entries.remove(index);
        self.mark_pages(page_extent.start_page, page_extent.len_pages, false);

        // No need to clear data from storage, can be overwritten.
        Ok(())
    }

    pub fn entries(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries.iter()
    }

    // Dump / Restore
    pub fn dump<W: FnMut(&[u8])>(&self, mut write: W) -> Result<(), FsErr> {
        write(b"MEMFS"); // Magic
        write(&[1u8]); // Version
        write(&PAGE_SIZE.to_le_bytes());

        let num_pages: u32 = Self::num_pages() as u32;
        write(&num_pages.to_le_bytes());

        // Entries
        let entry_count: u32 = self.entries.len() as u32;
        write(&entry_count.to_le_bytes());

        for file in &self.entries {
            let name_bytes = file.name.as_str().as_bytes();
            let name_len: u16 = name_bytes
                .len()
                .try_into()
                .map_err(|_| FsErr::FileNameInvalid("DUMP"))?; // or NameTooLong
            write(&name_len.to_le_bytes());
            write(name_bytes);

            write(&(file.size as u32).to_le_bytes());
            write(&file.flags.bits().to_le_bytes());
            write(&(file.extent.start_page as u32).to_le_bytes());
            write(&(file.extent.len_pages as u32).to_le_bytes());
        }

        // Page bitmap
        let bm_len: u32 = self.page_bitmap.len() as u32;
        write(&bm_len.to_le_bytes());
        for line in &self.page_bitmap {
            write(&line.to_le_bytes());
        }

        // Data
        let storage_len: u32 = self.storage.len() as u32;
        write(&storage_len.to_le_bytes());
        write(&self.storage);

        Ok(())
    }

    // Page allocator functions
    fn page_is_free(&self, page: usize) -> bool {
        (self.page_bitmap[page / 32] & (1 << (page % 32))) == 0
    }
    fn mark_pages(&mut self, start: usize, len: usize, used: bool) {
        for page in start..start + len {
            let page_bit = &mut self.page_bitmap[page / 32];
            let bit = 1 << (page % 32);
            if used {
                *page_bit |= bit;
            } else {
                *page_bit &= !bit;
            }
        }
    }

    // First-fit run search.
    fn find_free_pages(&self, need_pages: usize) -> Option<Extent> {
        // TODO: Should this assert be here?
        // assert_ne!(need_pages, 0);
        let mut run_start = None; //TODO: Use previous alloction marker, potentially speeds up search.
        let mut run_len = 0;

        for page in 0..Self::num_pages() {
            if self.page_is_free(page) {
                if run_start.is_none() {
                    run_start = Some(page)
                }
                run_len += 1;
                if run_len >= need_pages {
                    return Some(Extent {
                        start_page: run_start.unwrap(),
                        len_pages: run_len,
                    });
                }
            } else {
                run_start = None;
                run_len = 0;
            }
        }
        None
    }
    // Check if next pages are free. Early return if not the case.
    fn check_neighbour_pages_free(&self, start: usize, need_pages: usize) -> Option<Extent> {
        assert!(start <= Self::num_pages());
        assert_ne!(need_pages, 0);
        let mut run_len = 0;

        for page in start..Self::num_pages() {
            if self.page_is_free(page) {
                run_len += 1;
            } else {
                return None;
            }
            if run_len >= need_pages {
                return Some(Extent {
                    start_page: start,
                    len_pages: run_len,
                });
            }
        }
        None
    }

    // Helper functions

    /// Check for invalid or duplicate names.
    fn validate_file_name(
        &self,
        name: String<MAX_FILE_NAME_LENGTH>,
    ) -> Result<String<MAX_FILE_NAME_LENGTH>, FsErr> {
        // Check for invalid or duplicate names.
        if name.is_empty() || name.contains(" ") {
            return Err(FsErr::FileNameInvalid(
                "File name cannot be empty or a whitespace.",
            ));
        }
        if self.entries.iter().any(|f| f.name == name) {
            return Err(FsErr::Duplicate);
        }

        Ok(name)
    }

    fn find_file_index(&self, name: &str) -> Result<usize, FsErr> {
        match self.entries.iter().position(|f| f.name == name) {
            Some(index) => Ok(index),
            None => Err(FsErr::NotFound),
        }
    }

    // Debug
    #[cfg(feature = "std")]
    pub fn list_files(&self) {
        println!("File entries:");
        for entry in &self.entries {
            println!(
                "\t{} ({} bytes @ {})",
                entry.name,
                entry.size,
                entry.extent.start_page * PAGE_SIZE
            );
        }
    }

    /// Visualize the filesystem in hex format.
    #[cfg(feature = "std")]
    pub fn hex_dump(&self, start: usize, len: usize) {
        let end = (start + len).min(STORAGE_SIZE);
        for (i, chunk) in self.storage[start..end].chunks(16).enumerate() {
            print!("{:#06x} | ", start + i * 16);
            for b in chunk {
                print!("{:02X} ", b);
            }
            print!(" | ");
            for b in chunk {
                let c = *b as char;
                if c.is_ascii_graphic() || c == ' ' {
                    print!("{}", c);
                } else {
                    print!(".");
                }
            }
            println!();
        }
    }
}
