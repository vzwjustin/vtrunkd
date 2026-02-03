## 2024-05-22 - [Loading State Accessibility]
**Learning:** When styling loading states in CSS, `color: transparent` hides `currentColor`, making borders invisible. Explicit `border-color` is required for each button variant. Also, `pointer-events: none` suppresses `cursor: wait`, so use `disabled` attribute instead.
**Action:** Always define specific border colors for spinners in different button variants and rely on `disabled` for interaction blocking.
