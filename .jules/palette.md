## 2024-05-21 - Loading States in Dark/Ghost Buttons
**Learning:** When hiding button text with `color: transparent` for a loading state, `currentColor` becomes transparent, making default borders invisible. Always explicitly set `border-color` for spinners on all button variants.
**Action:** Use specific CSS selectors for each button variant (.primary, .ghost, .danger) to enforce visible spinner colors.
