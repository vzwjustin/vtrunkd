## 2026-01-27 - Loading States for Async Actions
**Learning:** In a vanilla JS/Tauri app, button feedback is critical for long-running operations like provisioning. A simple `withLoading` wrapper that toggles a class and `disabled` state is an effective, reusable pattern that avoids complex state management libraries.
**Action:** Apply the `withLoading` wrapper pattern to any new async interaction buttons to ensure consistent feedback and prevent race conditions.
