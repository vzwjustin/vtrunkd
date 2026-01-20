## 2025-10-26 - WireGuard Buffer Allocations
**Learning:** The WireGuard hot path was allocating `Vec<u8>` for every packet processed to work around borrow checker constraints, even though downstream functions accepted `&[u8]`. Rust's NLL (Non-Lexical Lifetimes) is smart enough to handle re-borrowing if the previous borrow is dropped (e.g. after await), allowing us to remove these allocations.
**Action:** Always check if `.to_vec()` is strictly necessary when passing data to async functions that take slices.
