## 2024-05-23 - [Zero-Copy Packet Processing]
**Learning:** `vtrunkd` packet processing path allocated `Vec<u8>` for every packet due to `to_vec()` calls on `boringtun` results, which returns slices borrowing the output buffer. These allocations are unnecessary as `UdpSocket::send_to` accepts slices.
**Action:** Always check if `to_vec()` is necessary when bridging libraries; prefer passing slices (references) in hot paths. In async contexts, ensure borrows don't cross await points conflictingly.
