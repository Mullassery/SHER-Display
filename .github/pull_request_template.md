## What this changes

<!-- Describe the change. If it adds a new crate or changes ROADMAP.md status, say so explicitly. -->

## Boundary check

- [ ] This does not instantiate a driver or hardware handle already owned by SHER-Kernel or SHER-Graphics (see VISION.md's "Ownership boundaries").
- [ ] If this touches `input/`, it goes through `sher_input_core::InputService`, not a local reimplementation.

## Testing

- [ ] `cargo fmt -- --check` passes
- [ ] `cargo build --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] New behavior has a test. If not, explain why below.

## Docs

- [ ] `ROADMAP.md` status markers (`[x]`/`[~]`/`[ ]`) updated if this changes what's built.
- [ ] `README.md`'s crate list / claims updated if this adds/removes a crate or changes what's true.

## Notes for reviewers

<!-- Anything not covered above: known limitations of this PR, follow-up work intentionally left out, etc. -->
