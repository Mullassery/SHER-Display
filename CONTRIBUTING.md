# Contributing to SHER-Display

SHER-Display is an early-stage, pre-alpha compositor/window-management
subsystem (see [README.md](README.md), [VISION.md](VISION.md), and
[ROADMAP.md](ROADMAP.md) before contributing — they describe what's actually
built versus planned, and the architectural boundaries that must not be
violated).

## Before you start

- Read [VISION.md](VISION.md)'s "Ownership boundaries" section first. The
  single most common mistake in this codebase's history has been a crate
  instantiating a driver or hardware handle that SHER-Kernel or SHER-Graphics
  already owns (see the `GPUDriver` ownership decision documented there). If
  you're about to write `gpu_driver::GPUDriver::new(...)`,
  `input_driver::InputDriver::new(...)`, or anything that constructs a
  stateful handle to hardware another SHER subsystem already owns — stop and
  mirror facts instead.
- Read [ROADMAP.md](ROADMAP.md) to see what phase is currently active and
  what's explicitly "not started." Don't build ahead of the current phase
  without discussing it in an issue first — several structural decisions
  (the `crates/` layout migration, the `sher_display_backend` trait) are
  still open and unstarted work should not pre-empt them.
- This repo does not build standalone. You need `SHER-Kernel`,
  `SHER-Graphics`, and `SHER-Input` checked out as sibling directories (see
  README.md's "Building" section).

## Development setup

```bash
git clone https://github.com/Mullassery/SHER-KERNEL ../SHER-Kernel
git clone https://github.com/Mullassery/SHER-Graphics ../SHER-Graphics
git clone https://github.com/Mullassery/SHER-INPUT ../SHER-Input
git clone https://github.com/Mullassery/SHER-Display
cd SHER-Display
cargo build --workspace
cargo test --workspace
```

## Before opening a PR

Run the same checks CI runs, from the repo root:

```bash
cargo fmt -- --check
cargo build --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

All four must pass with zero warnings. `cargo fmt --all` is deliberately not
used (it would also reformat the sibling path-dependency repos) — use plain
`cargo fmt`.

## What to include in a PR

- New code needs tests. Every crate in this workspace currently has direct
  unit test coverage; a PR that adds behavior without a test for it will be
  asked to add one.
- If your change adds a new crate, update `Cargo.toml`'s `members` list and
  the crate list in `README.md`, `ROADMAP.md`, and `VISION.md` as
  applicable — this repo tracks status honestly (`[x]`/`[~]`/`[ ]` in
  `ROADMAP.md`) and stale status markers are treated as bugs.
- Do not claim something works in a doc comment or README if it isn't
  covered by a test. If you're not sure whether something is tested, say so
  in the PR description rather than asserting it works.

## Reporting bugs / requesting features

Use the issue templates under `.github/ISSUE_TEMPLATE/`. There is no
mailing list or chat for this project yet — GitHub issues are the only
channel.

## Security issues

Do not open a public issue for a security concern — see
[SECURITY.md](SECURITY.md).

## License

By contributing, you agree your contributions are licensed under the
[Apache License 2.0](LICENSE), matching the rest of the repo.
