#![cfg_attr(not(feature = "std"), no_std)]

use core::str::FromStr;
use crc::{CRC_32_CKSUM, Crc, NoTable};
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
        const APPEND_ONLY=1<<3; // no write_at
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

impl FileEntry {
    const fn serialized_max_size() -> usize {
        18 + MAX_FILE_NAME_LENGTH
    }
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

    /// Replace the entire contents of a file.
    ///
    /// Will create the file if it doesn't exsist yet.
    pub fn write(&mut self, name: &str, data: &[u8]) -> Result<(), FsErr> {
        if let Ok(index) = self.find_file_index(name) {
            // Check file flags
            if self.entries[index].flags.contains(FileFlags::IMMUTABLE) {
                return Err(FsErr::ReadOnly);
            }

            let required_pages = data.len().div_ceil(PAGE_SIZE);

            // Find new extent if needed
            if required_pages > self.entries[index].extent.len_pages {
                // Unmark old pages
                let old_extent = self.entries[index].extent;
                self.mark_pages(old_extent.start_page, old_extent.len_pages, false);

                let extent = self.find_free_pages(required_pages);

                if let Some(extent) = extent {
                    self.mark_pages(extent.start_page, extent.len_pages, true);

                    self.entries[index].extent = extent;
                    self.entries[index].size = data.len();

                    let offset = extent.start_page * PAGE_SIZE;
                    self.storage[offset..offset + data.len()].copy_from_slice(data);
                } else {
                    // Search failed, remark pages.
                    self.mark_pages(old_extent.start_page, old_extent.len_pages, true);

                    return Err(FsErr::NoSpace);
                }
            } else {
                self.entries[index].size = data.len();

                let offset = self.entries[index].extent.start_page * PAGE_SIZE;
                self.storage[offset..offset + data.len()].copy_from_slice(data);
            };

            return Ok(());
        } else {
            // File does not exsist.
            return self.create(name, data);
        }
    }

    /// Write content to a exsisting file at a given offset.
    ///
    /// If data exceeds currently allocated size, the file will grow in place.
    pub fn write_at(&mut self, name: &str, offset: usize, data: &[u8]) -> Result<(), FsErr> {
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

        // No hole check
        if offset > entry.size || entry.flags.contains(FileFlags::APPEND_ONLY) {
            return Err(FsErr::InvalidOp);
        }

        let current_extent = self.entries[index].extent;
        let current_capacity = current_extent.len_pages * PAGE_SIZE;

        let write_end = offset + data.len();
        // Try growing into neighbouring space.
        if write_end > current_capacity {
            let required_pages = write_end.div_ceil(PAGE_SIZE);
            let extra_pages = required_pages - current_extent.len_pages;

            assert!(extra_pages > 0);

            if let Some(neighbour_extent) = self.check_neighbour_pages_free(
                current_extent.start_page + current_extent.len_pages,
                extra_pages,
            ) {
                self.mark_pages(
                    neighbour_extent.start_page,
                    neighbour_extent.len_pages,
                    true,
                );

                self.entries[index].extent.len_pages += neighbour_extent.len_pages;
            } else {
                return Err(FsErr::WouldFragment);
            }
        }

        let start = current_extent.start_page * PAGE_SIZE + offset;

        self.storage[start..start + data.len()].copy_from_slice(data);
        self.entries[index].size = self.entries[index].size.max(offset + data.len());

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

    pub fn truncate(&mut self, name: &str, new_size: usize) -> Result<(), FsErr> {
        // Find file and check flags.
        let index = self.find_file_index(name)?;
        let entry = &self.entries[index];

        if entry.flags.contains(FileFlags::IMMUTABLE) {
            return Err(FsErr::ReadOnly);
        }

        if new_size > entry.size || new_size == 0 {
            return Err(FsErr::InvalidOp);
        }

        // Free unused pages
        let current_pages = entry.extent.len_pages;
        let required_pages = new_size.div_ceil(PAGE_SIZE);

        if required_pages < current_pages {
            let unused = Extent {
                start_page: entry.extent.start_page + required_pages,
                len_pages: entry.extent.len_pages - required_pages,
            };
            self.mark_pages(unused.start_page, unused.len_pages, false);
        }

        self.entries[index].extent = Extent {
            start_page: self.entries[index].extent.start_page,
            len_pages: required_pages,
        };
        self.entries[index].size = new_size;

        Ok(())
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
    const fn serialized_header_size() -> usize {
        5  // "MEMFS"
        + 1  // version
        + 4  // page size (u32)
        + 4  // num_pages (u32)
        + 4 // entry_count (u32)
    }

    const fn serialized_footer_size() -> usize {
        8 // "MEMFSEND"
        + 4 // total_len (u32)
        + 4 // checksum (u32)
    }

    pub const fn serialized_max_size() -> usize {
        Self::serialized_header_size()
        + MAX_NUM_FILES * FileEntry::serialized_max_size()
        + 4 // storage_len (u32)
        + STORAGE_SIZE
        + Self::serialized_footer_size()
    }

    pub fn dump<W: FnMut(&[u8])>(&self, mut write: W) -> Result<(), FsErr> {
        let crc = Crc::<u32, NoTable>::new(&CRC_32_CKSUM);
        let mut digest = crc.digest();
        let mut total_len: u32 = 0;

        {
            let mut write = |bytes: &[u8]| {
                digest.update(bytes);
                write(bytes);
                total_len = total_len.wrapping_add(bytes.len() as u32);
            };

            // Header
            write(b"MEMFS"); // Magic
            write(&[2u8]); // Version
            write(&(PAGE_SIZE as u32).to_le_bytes());

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
                    .map_err(|_| FsErr::FileNameInvalid("Invalid filename"))?;
                write(&name_len.to_le_bytes());
                write(name_bytes);

                write(&(file.size as u32).to_le_bytes());
                write(&file.flags.bits().to_le_bytes());
                write(&(file.extent.start_page as u32).to_le_bytes());
                write(&(file.extent.len_pages as u32).to_le_bytes());
            }

            // Data
            let storage_len: u32 = self.storage.len() as u32;
            write(&storage_len.to_le_bytes());
            write(&self.storage);
        }

        // Footer
        write(b"MEMFSEND");
        write(&total_len.to_le_bytes());
        write(&digest.finalize().to_le_bytes());

        Ok(())
    }

    pub fn restore<R>(&mut self, mut read: R) -> Result<(), FsErr>
    where
        R: FnMut(&mut [u8]) -> Result<(), FsErr>,
    {
        let crc = Crc::<u32, NoTable>::new(&CRC_32_CKSUM);
        let mut digest = crc.digest();
        let mut total_len: u32 = 0;

        {
            let mut read = |buf: &mut [u8]| -> Result<(), FsErr> {
                read(buf)?;
                digest.update(buf);
                total_len = total_len.wrapping_add(buf.len() as u32);
                Ok(())
            };

            let mut magic = [0u8; 5];
            let mut version = [0u8; 1];

            read(&mut magic)?;
            read(&mut version)?;

            // Validate Header
            if &magic != b"MEMFS" || version[0] != 2 {
                return Err(FsErr::Corrupt);
            }

            // Validate Sizes
            let mut page_size = [0u8; size_of::<u32>()];
            read(&mut page_size)?;
            let page_size = u32::from_le_bytes(page_size);
            if page_size as usize != PAGE_SIZE {
                return Err(FsErr::Corrupt);
            }

            let mut num_pages = [0u8; size_of::<u32>()];
            let mut num_entries = [0u8; size_of::<u32>()];

            read(&mut num_pages)?;
            read(&mut num_entries)?;

            let num_pages = u32::from_le_bytes(num_pages);
            let num_entries = u32::from_le_bytes(num_entries);

            if num_pages as usize != Self::num_pages() {
                return Err(FsErr::Corrupt);
            }

            if !self.entries.is_empty() {
                return Err(FsErr::InvalidOp);
            }

            for _ in 0..num_entries {
                let mut name_len = [0u8; size_of::<u16>()];
                let mut name_bytes = [0u8; MAX_FILE_NAME_LENGTH];

                read(&mut name_len)?;
                let name_len = u16::from_le_bytes(name_len) as usize;
                if name_len == 0 || name_len > MAX_FILE_NAME_LENGTH {
                    return Err(FsErr::Corrupt);
                }
                read(&mut name_bytes[..name_len])?;

                let name = str::from_utf8(&name_bytes[..name_len]).map_err(|_| FsErr::Corrupt)?;

                let mut file_size = [0u8; size_of::<u32>()];
                let mut file_flags = [0u8; size_of::<u32>()];
                let mut file_extent_start = [0u8; size_of::<u32>()];
                let mut file_extent_len = [0u8; size_of::<u32>()];

                read(&mut file_size)?;
                read(&mut file_flags)?;
                read(&mut file_extent_start)?;
                read(&mut file_extent_len)?;

                let file_size = u32::from_le_bytes(file_size);
                let file_flags = u32::from_le_bytes(file_flags);
                let file_extent_start = u32::from_le_bytes(file_extent_start) as usize;
                let file_extent_len = u32::from_le_bytes(file_extent_len) as usize;

                // Sanity checks
                if file_extent_len == 0 {
                    return Err(FsErr::Corrupt);
                }
                let cap = file_extent_len
                    .checked_mul(PAGE_SIZE)
                    .ok_or(FsErr::Corrupt)?;
                if (file_size as usize) > cap {
                    return Err(FsErr::Corrupt);
                }
                let end = file_extent_start
                    .checked_add(file_extent_len)
                    .ok_or(FsErr::Corrupt)?;
                if end > num_pages as usize {
                    return Err(FsErr::Corrupt);
                }

                self.entries
                    .push(FileEntry {
                        name: heapless::String::from_str(name).map_err(|_| FsErr::Corrupt)?,
                        size: file_size as usize,
                        flags: FileFlags::from_bits_truncate(file_flags),
                        extent: Extent {
                            start_page: file_extent_start,
                            len_pages: file_extent_len,
                        },
                    })
                    .map_err(|_| FsErr::Corrupt)?;

                self.mark_pages(file_extent_start, file_extent_len, true);
            }

            // Storage data
            let mut storage_len = [0u8; size_of::<u32>()];
            read(&mut storage_len)?;
            let storage_len = u32::from_le_bytes(storage_len) as usize;
            if storage_len != STORAGE_SIZE {
                return Err(FsErr::Corrupt);
            }
            read(&mut self.storage[..storage_len])?;
        }

        let mut footer_magic = [0u8; 8];
        let mut footer_len = [0u8; size_of::<u32>()];
        let mut footer_crc = [0u8; size_of::<u32>()];

        read(&mut footer_magic)?;
        read(&mut footer_len)?;
        read(&mut footer_crc)?;

        if &footer_magic != b"MEMFSEND" {
            return Err(FsErr::Corrupt);
        }

        let expected_len = u32::from_le_bytes(footer_len);
        if expected_len != total_len {
            return Err(FsErr::Corrupt);
        }

        let expected_crc = u32::from_le_bytes(footer_crc);
        let actual_crc = digest.finalize();
        if expected_crc != actual_crc {
            return Err(FsErr::Corrupt);
        }

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
        assert_ne!(need_pages, 0);

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
