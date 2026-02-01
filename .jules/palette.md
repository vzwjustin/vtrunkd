## 2024-05-22 - Visual Feedback for Async Actions
**Learning:** Users lack confidence in async operations (like "Generate Configs") without immediate visual feedback, leading to potential double-submissions or confusion.
**Action:** Implement a standard `withLoading` wrapper for all async handlers that applies a `.loading` class (spinner) and handles the `disabled` state via `try/finally` blocks to ensure recovery.
