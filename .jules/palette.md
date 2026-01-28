## 2024-05-23 - Async Loading States Pattern
**Learning:** Asynchronous GUI interactions are managed by a `withLoading` helper function in `main.js` which applies a `.loading` CSS class. Event handlers must use `e.currentTarget` (not `e.target`) to safely identify the button when children elements are present.
**Action:** When adding new async buttons, wrap the event handler in `withLoading(e.currentTarget, fn)` and ensure `.loading` styles in CSS handle the button's color scheme (e.g., specific spinner colors for ghost/danger variants).

## 2024-05-23 - Loading Spinner Visibility
**Learning:** When styling loading states in CSS where text is hidden via `color: transparent`, explicit border colors must be used for spinners (via `--spinner-color`) instead of `currentColor` to ensure visibility.
**Action:** Explicitly set `border-top-color` in CSS for loading spinners, especially when the text color is made transparent.

## 2024-05-23 - Verifying Tauri Loading States in Playwright
**Learning:** Verifying Tauri 1.x frontend logic in Playwright requires mocking `window.__TAURI_IPC__`. Crucially, to resolve the promise returned by `invoke`, the mock must extract the `callback` ID from the message and execute `window['_' + callback](response)`.
**Action:** Use this specific mock pattern for all future frontend integration tests involving Tauri IPC.
