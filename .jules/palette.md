## 2024-10-24 - Zero-Layout-Shift Loading Buttons
**Learning:** Using `color: transparent` on a button text combined with an absolute positioned spinner in `::after` allows for adding a loading state without causing layout shifts or requiring HTML structure changes (like wrapping text in a span).
**Action:** Use this pattern for retrofitting loading states onto existing buttons in vanilla JS/CSS projects where DOM modification is risky or tedious.
