## 2024-10-27 - Atomic Overhead in Hot Paths
**Learning:** In async Rust hot paths (like packet sending loops), unnecessary `Arc::clone` introduces measurable atomic overhead. Rust's NLL often allows replacing `Arc::clone` with a simple reference `&`, even across `.await` points, as long as the mutable borrow of `self` happens *after* the future completes.
**Action:** Audit hot loops for `Arc::clone` usage where a reference would suffice.
