## 2024-05-22 - Avoid redundant Arc clones in async hot paths
**Learning:** In async methods taking `&mut self`, cloning an `Arc` field to call an async method on it (like `UdpSocket::send_to`) is often redundant if `self`'s lifetime covers the future's duration.
**Action:** Check if `Arc::clone` can be removed by borrowing directly, verifying that the borrow ends before any subsequent mutable access to `self`.
