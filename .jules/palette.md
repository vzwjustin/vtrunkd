## 2024-05-22 - Async Action Feedback
**Learning:** Users often click buttons multiple times if there's no immediate visual feedback for async operations (like provisioning or generating configs), leading to potential race conditions or confusion.
**Action:** Always implement a distinct loading state (spinner + disabled) for primary action buttons that trigger network or async tasks.
