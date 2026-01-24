# Palette's Journal

## 2026-01-24 - Missing Async Feedback
**Learning:** The application performs several asynchronous operations (config generation, provisioning, tunnel start/stop) without visual feedback on the trigger buttons, leading to potential double-clicks and uncertainty.
**Action:** Implemented a reusable `withLoading` pattern that applies a spinner and disables the button during async calls.
**Technical Note:** When using `color: transparent` to hide text during loading, the spinner's border color must be defined explicitly (e.g., `var(--teal)`). Using `currentColor` will inherit the transparency, making the spinner invisible.
