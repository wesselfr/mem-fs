#[cfg(test)]
mod tests {
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
