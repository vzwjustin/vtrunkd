## 2026-01-30 - [Unified Loading States in Vanilla JS]
**Learning:** In vanilla JS apps, manually toggling loading classes in every async function leads to code duplication and inconsistency.
**Action:** Use a higher-order wrapper function (like `withLoading`) to automatically manage visual states (`.loading` class) and accessibility attributes (`aria-busy`) for all async interactions.
