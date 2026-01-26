#[cfg(test)]
mod test;

#[cfg(feature = "std")]
fn main() {
    use mem_fs::MemFs;

    let mut fs = MemFs::new();
    fs.create("hello.txt", b"hello mem-fs!").unwrap();

    let data = fs.read("hello.txt").unwrap();
    println!("{}", core::str::from_utf8(data).unwrap());

    fs.create("other_file.txt", b"some other data here.")
        .unwrap();

    fs.list_files();
    fs.hex_dump(0, 256);

    fs.delete("hello.txt").unwrap();

    fs.list_files();
    fs.hex_dump(0, 256);

    fs.append("other_file.txt", b" Pretty cool!").unwrap();

    fs.create(
        "yet_another_file.txt",
        b"some more data. some more data. some more data.",
    )
    .unwrap();

    fs.list_files();
    fs.hex_dump(0, 256);

    fs.append("yet_another_file.txt", b" We can add even more!")
        .unwrap();

    fs.append(
        "other_file.txt",
        b" Even some more extra data for other file",
    )
    .unwrap();

    fs.create("test_file.txt", b"some data").unwrap();
    fs.list_files();
    fs.hex_dump(0, 256);
}

#[cfg(not(feature = "std"))]
fn main() {
    // main is empty when building in no_std mode
}
