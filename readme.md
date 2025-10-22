# Mem-FS
A minimal in-memory file system for embedded and game applications.

mem-fs is a lightweight, high-performance in-memory file system written in Rust.
It’s designed for use in embedded systems, robotics, and game engines — any environment where fast, deterministic memory access matters more than complex storage semantics.

The goal is to provide a tiny, self-contained file system abstraction that can:
- Store files directly in memory (RAM or a preallocated buffer).
- Operate without dynamic allocations (no_std-friendly).
- Optionally dump its state to disk or flash memory for persistence.
- Be easily embedded into a larger runtime (e.g., asset loader, firmware storage).
