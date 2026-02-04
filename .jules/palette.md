## 2026-02-04 - Loading State implementation
**Learning:** When implementing loading states on buttons by making text transparent (`color: transparent`), the `currentColor` used for borders also becomes transparent.
**Action:** Explicitly set `border-color` for spinners on each button variant (primary, ghost, danger) to ensure visibility against the background.
