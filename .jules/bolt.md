## 2024-05-22 - WireGuard Packet Allocation & Arc Overhead
**Learning:** `boringtun`'s `TunnResult` returns slices that borrow from the output buffer. Allocating `Vec<u8>` from these slices (via `.to_vec()`) for immediate transmission is a significant performance bottleneck. Also, `Arc::clone` in hot paths (`send_to_link`) is unnecessary when `&mut self` guarantees exclusive access and the borrow outlives the await.
**Action:** Always check if a slice can be passed directly instead of allocating. Use references instead of `Arc::clone` when lifetimes permit.
