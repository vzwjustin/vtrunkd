## 2026-02-05 - Testing Loading States in Tauri
**Learning:** When verifying async loading states in a Tauri app using Playwright, simply mocking `window.__TAURI_IPC__` to return a value isn't enough. You must explicitly execute the callback function (e.g., `window['_' + callbackId]`) to resolve the frontend Promise, otherwise the UI remains stuck in the loading state (e.g. `finally` blocks never run).
**Action:** Always include callback resolution logic in Tauri IPC mocks to ensure full lifecycle testing of async UI components.
