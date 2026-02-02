## 2024-05-23 - Avoiding Allocations with boringtun TunnResult
**Learning:** `boringtun`'s `TunnResult` borrows the output buffer, which often forces developers to clone the data (allocate `Vec<u8>`) to release the borrow before calling `decapsulate` again in a loop.
**Action:** Use a `should_continue` variable to exit the `match` block (dropping the borrow) before looping, allowing zero-copy reuse of the buffer.
