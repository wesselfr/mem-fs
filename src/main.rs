#[cfg(feature = "std")]
fn main() {
    use mem_fs::MemoryFs;

    let mut fs = MemoryFs::new();
    fs.create("hello.txt", b"hello mem-fs!").unwrap();

    let data = fs.read("hello.txt").unwrap();
    println!("{}", core::str::from_utf8(data).unwrap());

    fs.create("other_file.txt", b"some other data here.")
        .unwrap();

    fs.list_files();
}

#[cfg(not(feature = "std"))]
fn main() {
    // main is empty when building in no_std mode
}
