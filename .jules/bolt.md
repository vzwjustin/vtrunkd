## 2024-05-22 - Redundant Arc Clone in LinkManager Hot Path
**Learning:** `LinkManager` async methods (like `send_to_link`) that take `&mut self` can often borrow inner `Arc` fields directly across `.await` points without cloning, provided the mutable borrow of `self` isn't needed until *after* the future completes. Removing `Arc::clone` saves atomic operations in the hot path.
**Action:** When optimizing async hot paths, audit `Arc::clone` usage. If the resulting future is awaited immediately and no conflicting borrows occur during the await, remove the clone.
