## 2024-03-22 - Async Loading States
**Learning:** Users lack feedback during long-running async operations like "Generate configs" or "Provision VPS", leading to uncertainty. Adding a standardized `withLoading` helper that disables the button and shows a spinner (using explicit border colors for visibility on transparent text) provides consistent and immediate feedback.
**Action:** Always wrap async action handlers in a `withLoading` or similar state management function.
