## 2024-05-23 - Async Loading Patterns
**Learning:** The vanilla JS frontend lacked a standard way to handle async loading states, leading to potential user confusion during long-running operations.
**Action:** Introduced a reusable `withLoading(id, fn)` helper and a CSS-only spinner pattern. Future async UI interactions should wrap their handlers with this helper to automatically manage disabled states and visual feedback.
