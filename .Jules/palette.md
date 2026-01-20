## 2025-10-26 - Loading States for Buttons
**Learning:** The design system uses `button` elements with various classes (`primary`, `danger`, `ghost`). For loading states where text is hidden, using `currentColor` for spinners fails because the text is transparent.
**Action:** Always use explicit border colors matching the button variant (e.g., `#fff` for primary, `var(--teal)` for ghost) when creating CSS-only spinners.
