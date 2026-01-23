## 2025-02-21 - [Zero-Copy Constraints]
**Learning:** The `NetPacket` struct used in `setup_links` requires data ownership (`Vec<u8>`) for channel transmission, preventing zero-copy optimizations in that specific scope. However, the send path (`run` loop and `handle_incoming`) can utilize `&[u8]` slices directly, avoiding allocations.
**Action:** When optimizing packet handling, focus on the send path where ownership is not required. For the receive path, consider refactoring `NetPacket` or using shared ownership like `Bytes` if further optimization is needed.
