# mem-fs
A minimal, deterministic in-memory file system for embedded and systems projects.

`mem-fs` is a lightweight, high-performance in-memory file system written in Rust.
It is designed for environments where **predictable memory usage, fast access, and tight control over allocation** matter more than complex storage semantics.

Typical use-cases include:

- embedded systems & firmware
- robotics runtimes
- game engines and simulation tooling
- small experimental “OS” or application platforms

---

## ✨ Goals
`mem-fs` focuses on predictable memory usage and simple in-memory storage,
rather than full persistent filesystem semantics.

`mem-fs` aims to provide a tiny, self-contained file system abstraction that can:

- Store files directly in memory (RAM or a user-provided buffer)
- Work in `no_std` environments
- Avoid dynamic allocation where possible
- Provide deterministic, inspectable memory layout
- Be embedded into a larger runtime (asset system, scripting VM, firmware storage, etc.)
- Optionally support dumping/loading its state for persistence

---

## 🚫 Non-goals
This crate intentionally does **not** aim to be:

- POSIX compliant
- a replacement for persistent filesystems (FAT, LittleFS, ext4, …)
- feature-rich in permissions, users, or security models
- crash-safe or journaling (at least not in early versions)

`mem-fs` is about **control, simplicity, and predictability**, not completeness.

---

## 🧪 Status
`mem-fs` is **experimental** and under active development.

- APIs may change.
- Internal layout and structures are still evolving.
- The crate is primarily developed as part of a larger embedded “pocket-computer” project.

That said, the project builds cleanly and is meant to grow into a solid, reusable component.

---

## 📦 Example
```rust
use mem_fs::MemFs;

let mut fs = MemFs::new();

fs.create("hello.txt", b"Hello mem-fs!").unwrap();
fs.append("hello.txt", b" Pretty cool!").unwrap();

let data = fs.read("hello.txt").unwrap();
```
