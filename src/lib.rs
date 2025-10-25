#![cfg_attr(not(feature = "std"), no_std)]

use heapless::{String, Vec};

const MAX_FILE_NAME_LENGHT: usize = 255;
const MAX_NUM_FILES: usize = 32;

const STORAGE_SIZE: usize = 4096;

const PAGE_SIZE: usize = 128;
const NUM_PAGES: usize = STORAGE_SIZE / PAGE_SIZE;

struct Extent {
    // TODO: Consider u16 / u32 for start_page and len_page.
    start_page: usize,
    len_pages: usize,
}

pub struct FileEntry {
    pub name: String<MAX_FILE_NAME_LENGHT>,
    pub offset: usize,
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
        todo!()
    }
    pub fn read(&self, name: &str) -> Option<&[u8]> {
        todo!()
    }
    pub fn delete(&mut self, name: &str) -> Result<(), &'static str> {
        todo!()
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
        todo!()
    }

    /// Visualize the filesystem in hex format.
    pub fn hex_dump(&self) {
        todo!()
    }
}
