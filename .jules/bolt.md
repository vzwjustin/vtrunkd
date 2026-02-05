## 2024-05-22 - [Redundant Allocations in Hot Path]
**Learning:** The codebase contained multiple instances of `Vec<u8>` allocation (via `.to_vec()`) solely to pass data to async functions accepting `&[u8]`. This is a common anti-pattern when interfacing with libraries like `boringtun` that return mutable slices.
**Action:** Always check if an async function accepting `&[u8]` can accept the slice directly from the source (e.g. `boringtun::TunnResult`) without cloning, relying on the borrow checker to validate lifetimes across the await point.
