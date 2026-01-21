## 2026-01-21 - Loading State Pattern
**Learning:** For buttons with `color: transparent` (to hide text during loading), `currentColor` cannot be used for the spinner border as it also becomes transparent.
**Action:** When implementing loading states on buttons, define spinner colors explicitly based on the button variant (e.g., primary: white, ghost: teal) rather than relying on `currentColor`.
