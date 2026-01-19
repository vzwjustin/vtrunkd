## 2025-05-23 - Allocations in Async Loop
**Learning:** `boringtun` returns buffers that borrow the input buffer. When passing these to async functions (like `links.send_packet`), using `.to_vec()` to satisfy the borrow checker is often unnecessary if the async function takes a slice and the buffer outlives the await.
**Action:** Always check if `.to_vec()` is truly needed to extend lifetime or if the scope naturally allows borrowing.
