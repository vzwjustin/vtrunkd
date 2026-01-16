Debug task: Modernize vtrunkd to a 2026 quality bar (fmt/clippy/tests/config safety/docs)

Context:
- Failing command: cargo fmt --check
- Expected behavior: formatting clean and quality gates pass
- Observed failure: rustfmt reports diffs; clippy also fails on a lint
- Constraints: Rust 2021, keep runtime behavior stable, preserve CLI and config compatibility

Acceptance criteria:
- cargo fmt --check passes
- cargo clippy -- -D warnings passes
- cargo test passes with new unit tests covering config validation and bonding/control packet helpers
- Config parsing rejects unknown fields and bonding_mode is typed with alias support (bonding/bonded)
- README documents client/server requirement and bonding mode guidance

Loop rules:
- Keep this prompt unchanged each iteration.
- Run the smallest test that validates the hypothesis.
- Record an iteration log.
- Output <promise>FIXED</promise> only when all criteria are true.
- Stop after 6 iterations and summarize if still failing.
