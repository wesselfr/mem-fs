#![cfg_attr(not(feature = "std"), no_std)]

use core::str::FromStr;
use heapless::{String, Vec};

const MAX_FILE_NAME_LENGTH: usize = 255;
const MAX_NUM_FILES: usize = 32;

pub const STORAGE_SIZE: usize = 4096;

const PAGE_SIZE: usize = 32;
const NUM_PAGES: usize = STORAGE_SIZE / PAGE_SIZE;

#[derive(Copy, Clone)]
struct Extent {
    // TODO: Consider u16 / u32 for start_page and len_page.
    start_page: usize,
    len_pages: usize,
}

pub struct FileEntry {
    pub name: String<MAX_FILE_NAME_LENGTH>,
    pub size: usize,
    extent: Extent,
}

pub struct MemoryFs {
    pub entries: Vec<FileEntry, MAX_NUM_FILES>,
    pub storage: [u8; STORAGE_SIZE],
    page_bitmap: [u32; NUM_PAGES.div_ceil(32)],
}

impl MemoryFs {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            storage: [0; STORAGE_SIZE],
            page_bitmap: [0; NUM_PAGES.div_ceil(32)],
        }
    }

    // File system operations
    // TODO: Implement a filesystem trait for these functions
    // TODO: Support atomic operations
    pub fn create(&mut self, name: &str, data: &[u8]) -> Result<(), &'static str> {
        // Check if we have space for another entry
        if name.len() > MAX_FILE_NAME_LENGTH {
            return Err("Filename is too big");
        }

        let required_pages = data.len().div_ceil(PAGE_SIZE);
        let extent = self.find_free_pages(required_pages);

        if extent.is_none() {
            return Err("No free pages found");
        };
        let extent = extent.unwrap();

        let file_name: String<MAX_FILE_NAME_LENGTH> =
            String::from_str(name).expect("Error while processing filename");

        // Check for invalid or duplicate names.
        if file_name == "" || file_name == " " {
            return Err("File name cannot be empty or a whitespace.");
        }
        if self
            .entries
            .iter()
            .position(|f| f.name == file_name)
            .is_some()
        {
            return Err("File already exsist.");
        }

        self.entries
            .push(FileEntry {
                name: file_name,
                size: data.len(),
                extent,
            })
            // FIXME: FileEntry should not be a limiting factor for adding files, storage space should be the only limit.
            .map_err(|_| "Too many files")?;

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
    pub fn delete(&mut self, name: &str) -> Result<(), &'static str> {
        let index = match self.entries.iter().position(|f| f.name == name) {
            Some(index) => index,
            None => return Err("File not found."),
        };
        let page_extent = self.entries[index].extent;

        self.entries.remove(index);
        self.mark_pages(page_extent.start_page, page_extent.len_pages, false);

        // No need to clear data from storage, can be overwritten.
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
        let mut run_start = None; //TODO: Use previous alloction marker, potentially speeds up search.
        let mut run_len = 0;

        for page in 0..NUM_PAGES {
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

    // Debug
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
    pub fn hex_dump(&self, start: usize, len: usize) {
        let end = (start + len).min(STORAGE_SIZE);
        for (i, chunk) in self.storage[start..end].chunks(16).enumerate() {
            #[cfg(feature = "std")]
            {
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
}
