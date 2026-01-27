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
        // FIXME: Use actual storage limit.
        let data = [255; mem_fs::DEFAULT_STORAGE_SIZE];
        fs.create("foo", &data).unwrap();
    }

    #[test]
    fn file_too_big() {
        let mut fs = MemFs::new();
        let data = [255; mem_fs::DEFAULT_STORAGE_SIZE + 1];
        assert!(fs.create("foo", &data).is_err());
    }

    #[cfg(not(feature = "std"))]
    #[test]
    fn no_std_builds() {
        let _ = mem_fs::MemFs::new();
    }
}
