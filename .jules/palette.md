## 2024-05-22 - Loading State Feedback
**Learning:** Adding a loading spinner that replaces text (via `color: transparent`) while maintaining button dimensions prevents layout shifts and provides clear feedback, but requires specific border colors for each button variant to ensure the spinner is visible against the background.
**Action:** Always verify spinner visibility against button background colors when using `currentColor` or specific colors.
