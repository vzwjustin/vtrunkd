## 2024-05-22 - Loading State Visibility
**Learning:** When styling loading states by hiding text with `color: transparent`, you cannot rely on `border-color: currentColor` for the spinner, as it will also be transparent.
**Action:** Always define explicit `--spinner-color` CSS variables for different button variants (primary, ghost, danger) to ensure visibility against the button background.
