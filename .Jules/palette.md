## 2025-02-16 - Dynamic Label Association
**Learning:** When cloning HTML templates for dynamic lists, static labels lose their input association.
**Action:** In the render loop, generate unique IDs (e.g., using index) and programmatically assign `id` to input and `for` to label.
