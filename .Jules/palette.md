## 2026-01-18 - Async Feedback Loops
**Learning:** In "dashboard" interfaces like vtrunkd, users often hesitate or double-click when complex backend operations (provisioning, generating keys) lack immediate visual feedback. The log output is often too disconnected from the action trigger.
**Action:** Always couple async button presses with an immediate disabled/loading state on the button itself to confirm receipt of the command and prevent re-submission.
