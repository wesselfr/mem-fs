#[cfg(test)]
mod tests {
    use mem_fs::FileFlags;
    use mem_fs::FsErr;
    use mem_fs::MemFs;

    #[test]
    fn create_read() {
        let mut fs = MemFs::new();
        fs.create("foo", b"test").expect("Failed to create file.");

        assert_eq!(fs.read("foo").unwrap(), b"test");
    }

    #[test]
    fn create_empty_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"").expect("Failed to create file.");

        assert_eq!(fs.read("foo").unwrap(), b"");
    }

    #[test]
    fn multiple_files() {
        let mut fs = MemFs::new();
        fs.create("foo", b"file_1").expect("Failed to create file.");
        fs.create("bar", b"file_2").expect("Failed to create file.");

        assert_eq!(fs.read("foo").unwrap(), b"file_1");
        assert_eq!(fs.read("bar").unwrap(), b"file_2");
    }

    #[test]
    fn iter_files() {
        let mut fs = MemFs::new();
        fs.create("foo", b"file_1").unwrap();
        fs.create("bar", b"file_2").unwrap();

        let entries: Vec<_> = fs.entries().collect();

        assert_eq!(entries.len(), 2);

        let names: Vec<_> = entries.iter().map(|f| f.name.as_str()).collect();

        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
    }

    #[test]
    fn file_exists() {
        let mut fs = MemFs::new();
        fs.create("foo", b"test").expect("Failed to create file.");
        assert_eq!(fs.exists("foo"), true);
    }

    #[test]
    fn file_not_existsing() {
        let fs = MemFs::new();
        assert_eq!(fs.exists("foo"), false);
    }

    #[test]
    fn read_non_existing_file() {
        let fs = MemFs::new();
        assert!(fs.read("foo").is_none());
    }

    #[test]
    fn empty_file_name() {
        let mut fs = MemFs::new();
        assert!(fs.create("", b"test").is_err());
    }

    #[test]
    fn create_duplicate_file() {
        let mut fs = MemFs::new();

        fs.create("foo", b"test").unwrap();
        assert!(fs.create("foo", b"test").is_err());
    }

    #[test]
    fn rename_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"test").expect("Failed to create file");
        assert_eq!(fs.read("foo").unwrap(), b"test");
        fs.rename("foo", "bar").expect("Failed to rename file.");
        assert!(fs.read("foo").is_none());
        assert_eq!(fs.read("bar").unwrap(), b"test");
    }

    #[test]
    fn rename_file_invalid() {
        let mut fs = MemFs::new();
        fs.create("foo", b"test").expect("Failed to create file");

        assert!(fs.rename("foo", "").is_err());
        assert!(fs.rename("foo", " ").is_err());
        assert!(fs.rename("foo", " bar").is_err());
        assert!(fs.rename("foo", "foo").is_err());
    }

    #[test]
    fn rename_file_duplicate() {
        let mut fs = MemFs::new();
        fs.create("a", b"file_1").expect("Failed to create file.");
        fs.create("b", b"file_2").expect("Failed to create file.");

        assert!(matches!(fs.rename("b", "a"), Err(FsErr::Duplicate)));
    }

    #[test]
    fn write_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"Hello").unwrap();
        fs.write("foo", b"World!").unwrap();

        assert_eq!(fs.read("foo").unwrap(), b"World!");
    }

    #[test]
    fn write_at_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"Hello World!").unwrap();
        fs.write_at("foo", 6, b"Rust!").unwrap();

        assert_eq!(fs.read("foo").unwrap(), b"Hello Rust!!");
    }

    #[test]
    fn write_respects_immutable_flag() {
        let mut fs = MemFs::new();

        fs.create_with_flags("foo", &[1u8; 10], FileFlags::IMMUTABLE)
            .unwrap();

        let err = fs.write("foo", &[2u8; 10]).unwrap_err();
        assert!(matches!(err, FsErr::ReadOnly));

        let err = fs.write_at("foo", 0, &[3u8; 1]).unwrap_err();
        assert!(matches!(err, FsErr::ReadOnly));
    }

    #[test]
    fn write_empty_frees_space_and_reads_empty() {
        let mut fs = MemFs::new();

        let big = [0xAAu8; mem_fs::DEFAULT_STORAGE_SIZE];
        fs.create("big", &big).unwrap();

        fs.write("big", b"").unwrap();
        assert_eq!(fs.read("big").unwrap(), b"");

        let big2 = [0xBBu8; mem_fs::DEFAULT_STORAGE_SIZE];
        fs.create("big2", &big2).unwrap();
        assert_eq!(fs.read("big2").unwrap(), &big2[..]);
    }

    #[test]
    fn write_at_empty_is_noop() {
        let mut fs = MemFs::new();
        fs.create("foo", &[1u8; 10]).unwrap();

        fs.write_at("foo", 0, &[]).unwrap();
        assert_eq!(fs.read("foo").unwrap(), &[1u8; 10]);
    }

    #[test]
    fn write_create_if_missing() {
        let mut fs = MemFs::new();
        fs.write("foo", &[4u8; 12]).unwrap();

        assert_eq!(fs.read("foo").unwrap(), &[4u8; 12]);
    }

    #[test]
    fn truncate_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"Hello World!").unwrap();
        fs.truncate("foo", 5).unwrap();

        assert_eq!(fs.read("foo").unwrap(), b"Hello");
    }

    #[test]
    fn append_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"").unwrap();
        fs.append("foo", b"test").unwrap();
    }

    #[test]
    fn append_non_exsisting_file() {
        let mut fs = MemFs::new();
        assert!(matches!(fs.append("foo", b"test"), Err(FsErr::NotFound)));
    }

    #[test]
    fn reserve_empty_file_allocates_capacity_but_size_stays_zero() {
        let mut fs = MemFs::new();
        fs.create("foo", b"").unwrap();

        assert_eq!(fs.read("foo").unwrap(), b"");
        assert_eq!(fs.capacity("foo").unwrap(), 0);

        fs.reserve("foo", 1).unwrap(); // should allocate 1 page
        let cap = fs.capacity("foo").unwrap();
        assert!(cap >= 1);
        assert!(cap.is_multiple_of(mem_fs::DEFAULT_PAGE_SIZE)); // assumes default MemFs

        // Still logically empty
        assert_eq!(fs.read("foo").unwrap(), b"");
    }

    #[test]
    fn reserve_does_not_change_file_size() {
        let mut fs = MemFs::new();
        fs.create("foo", b"hello").unwrap();

        let before = fs.read("foo").unwrap().to_vec();
        fs.reserve("foo", 200).unwrap(); // might grow capacity depending on layout

        assert_eq!(fs.read("foo").unwrap(), before.as_slice());
    }

    #[test]
    fn reserve_respects_immutable() {
        let mut fs = MemFs::new();
        fs.create_with_flags("foo", b"", FileFlags::IMMUTABLE)
            .unwrap();

        let err = fs.reserve("foo", 1).unwrap_err();
        assert!(matches!(err, FsErr::ReadOnly));

        let err = fs.reserve_or_repack("foo", 1).unwrap_err();
        assert!(matches!(err, FsErr::ReadOnly));
    }

    #[test]
    fn reserve_would_fragment_but_reserve_or_repack_succeeds() {
        let mut fs = MemFs::new();

        // Use small allocations that will likely end up adjacent with first-fit.
        fs.create("a", &[0x11; 1]).unwrap(); // 1 page
        fs.create("b", &[0x22; 1]).unwrap(); // 1 page, likely right after "a"

        // Now try to reserve extra pages for "a" so it needs to grow.
        // Neighbour page(s) should be occupied by "b", so strict reserve should fragment.
        let grow_to = mem_fs::DEFAULT_PAGE_SIZE * 3; // request 3 pages capacity
        let err = fs.reserve("a", grow_to).unwrap_err();
        assert!(matches!(err, FsErr::WouldFragment));

        // But repack is allowed; it can move "a" to a new contiguous run.
        fs.reserve_or_repack("a", grow_to).unwrap();

        // Contents must still be intact
        assert_eq!(fs.read("a").unwrap(), &[0x11; 1]);
    }

    #[test]
    fn reserve_fails_no_space_when_full() {
        let mut fs = MemFs::new();

        // Fill storage with one big file.
        let big = [0xAAu8; mem_fs::DEFAULT_STORAGE_SIZE];
        fs.create("big", &big).unwrap();

        // Create an empty file entry.
        fs.create("small", b"").unwrap();

        // Any reserve that needs pages should fail.
        let err = fs.reserve("small", 1).unwrap_err();
        assert!(matches!(err, FsErr::NoSpace));
    }

    #[test]
    fn delete_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"test").unwrap();
        fs.delete("foo").expect("Failed to delete file");
        assert!(fs.read("foo").is_none());
    }

    #[test]
    fn delete_non_existing_file() {
        let mut fs = MemFs::new();
        assert!(fs.delete("foo").is_err());
    }

    #[test]
    fn delete_empty_file() {
        let mut fs = MemFs::new();
        fs.create("foo", b"").unwrap();
        fs.delete("foo").unwrap();
    }

    #[test]
    fn delete_file_twice() {
        let mut fs = MemFs::new();
        fs.create("foo", b"test").unwrap();

        assert!(fs.delete("foo").is_ok());
        assert!(fs.delete("foo").is_err());
    }

    #[test]
    fn large_file() {
        let mut fs = MemFs::new();
        let data = [255; mem_fs::DEFAULT_STORAGE_SIZE];
        fs.create("foo", &data).unwrap();
    }

    #[test]
    fn file_too_big() {
        let mut fs = MemFs::new();
        let data = [255; mem_fs::DEFAULT_STORAGE_SIZE + 1];
        assert!(fs.create("foo", &data).is_err());
    }

    mod persistence {
        use mem_fs::DEFAULT_STORAGE_SIZE;
        use mem_fs::FsErr;
        use mem_fs::MemFs;

        fn dump_to_vec(fs: &MemFs) -> Vec<u8> {
            let mut out = Vec::new();
            fs.dump(|chunk| out.extend_from_slice(chunk)).unwrap();
            out
        }

        fn restore_from_slice(fs: &mut MemFs, data: &[u8]) -> Result<(), FsErr> {
            let mut pos = 0usize;
            fs.restore(|buf| {
                let end = pos + buf.len();
                if end > data.len() {
                    return Err(FsErr::Corrupt);
                }
                buf.copy_from_slice(&data[pos..end]);
                pos = end;
                Ok(())
            })
        }

        #[test]
        fn dump_restore_roundtrip_basic() {
            let mut fs = MemFs::new();
            fs.create("foo", b"hello").unwrap();
            fs.create("bar", b"world!!").unwrap();

            let data = dump_to_vec(&fs);

            let mut fs2 = MemFs::new();
            restore_from_slice(&mut fs2, &data).unwrap();

            assert_eq!(fs2.read("foo").unwrap(), b"hello");
            assert_eq!(fs2.read("bar").unwrap(), b"world!!");
        }

        #[test]
        fn dump_restore_empty_fs() {
            let fs = MemFs::new();
            let data = dump_to_vec(&fs);

            let mut fs2 = MemFs::new();
            restore_from_slice(&mut fs2, &data).unwrap();

            assert_eq!(fs2.entries().count(), 0);
        }

        #[test]
        fn restore_rejects_bad_magic() {
            let fs = MemFs::new();
            let mut data = dump_to_vec(&fs);

            data[0] ^= 0xFF; // corrupt first magic byte

            let mut fs2 = MemFs::new();
            assert!(matches!(
                restore_from_slice(&mut fs2, &data),
                Err(FsErr::Corrupt)
            ));
        }

        #[test]
        fn restore_rejects_truncated_dump() {
            let mut fs = MemFs::new();
            fs.create("foo", b"hello").unwrap();

            let data = dump_to_vec(&fs);
            let truncated = &data[..data.len() - 1];

            let mut fs2 = MemFs::new();
            assert!(restore_from_slice(&mut fs2, truncated).is_err());
        }

        #[test]
        fn restore_rejects_storage_len_mismatch() {
            let fs = MemFs::new();
            let mut data = dump_to_vec(&fs);

            // Patch storage_len (last 4 bytes before storage).
            // Since storage is the final block, storage_len starts at: data.len() - STORAGE_SIZE - 4
            let off = data.len() - DEFAULT_STORAGE_SIZE - 4;
            let bad = (DEFAULT_STORAGE_SIZE as u32 - 1).to_le_bytes();
            data[off..off + 4].copy_from_slice(&bad);

            let mut fs2 = MemFs::new();
            assert!(matches!(
                restore_from_slice(&mut fs2, &data),
                Err(FsErr::Corrupt)
            ));
        }
    }
    #[cfg(not(feature = "std"))]
    #[test]
    fn no_std_builds() {
        let _ = mem_fs::MemFs::new();
    }
}
