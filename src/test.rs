#[cfg(test)]
mod tests {
    use mem_fs::MemoryFs;

    #[test]
    fn create_read() {
        let mut fs = MemoryFs::new();
        fs.create("foo", b"test").expect("Failed to create file.");

        assert_eq!(fs.read("foo").unwrap(), b"test");
    }

    #[test]
    fn multiple_files() {
        let mut fs = MemoryFs::new();
        fs.create("foo", b"file_1").expect("Failed to create file.");
        fs.create("bar", b"file_2").expect("Failed to create file.");

        assert_eq!(fs.read("foo").unwrap(), b"file_1");
        assert_eq!(fs.read("bar").unwrap(), b"file_2");
    }

    #[test]
    fn read_non_exsisting_file() {
        let fs = MemoryFs::new();
        assert!(fs.read("foo").is_none());
    }

    #[test]
    fn empty_file_name() {
        let mut fs = MemoryFs::new();
        assert!(fs.create("", b"test").is_err());
    }

    #[test]
    fn create_duplicate_file() {
        let mut fs = MemoryFs::new();

        fs.create("foo", b"test").unwrap();
        assert!(fs.create("foo", b"test").is_err());
    }

    #[test]
    fn delete_file() {
        let mut fs = MemoryFs::new();
        fs.create("foo", b"test").unwrap();
        fs.delete("foo").expect("Failed to delete file");
        assert!(fs.read("foo").is_none());
    }

    #[test]
    fn delete_non_existing_file() {
        let mut fs = MemoryFs::new();
        assert!(fs.delete("foo").is_err());
    }

    #[test]
    fn delete_empty_file() {
        let mut fs = MemoryFs::new();
        fs.create("foo", b"").unwrap();
        fs.delete("foo").unwrap();
    }

    #[test]
    fn delete_file_twice() {
        let mut fs = MemoryFs::new();
        fs.create("foo", b"test").unwrap();

        assert!(fs.delete("foo").is_ok());
        assert!(fs.delete("foo").is_err());
    }

    #[test]
    fn large_file() {
        let mut fs = MemoryFs::new();
        // FIXME: Use actual storage limit.
        let data = [255; mem_fs::STORAGE_SIZE];
        fs.create("foo", &data).unwrap();
    }

    #[test]
    fn file_too_big() {
        let mut fs = MemoryFs::new();
        let data = [255; mem_fs::STORAGE_SIZE + 1];
        assert!(fs.create("foo", &data).is_err());
    }

    #[cfg(not(feature = "std"))]
    #[test]
    fn no_std_builds() {
        let _ = mem_fs::MemFs::new();
    }
}
