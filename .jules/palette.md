## 2026-01-31 - CSS Spinner with CurrentColor
**Learning:** When creating loading spinners for buttons with varying background colors, using `border-color: currentColor transparent currentColor transparent` allows the spinner to automatically adapt to the text color (e.g., white on primary buttons, teal on ghost buttons) without extra modifier classes.
**Action:** Use `currentColor` for icon and spinner colors in components to ensure they work seamlessly across different themes and variants.
