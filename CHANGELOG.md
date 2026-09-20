# Changelog

All notable changes to this project are documented in this file. Format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

This project has no tagged releases yet — everything below is unreleased,
pre-0.1.0, pre-alpha work. No version history is fabricated here; for the
full commit-by-commit history see `git log`.

## [Unreleased]

### Added since the initial scaffold
- Core compositor pipeline: `compositor`, `scene`, `surfaces`, `windows`,
  `workspaces`, `outputs`, `input`, `cursor` — 8 crates, in-memory /
  simulated state, unit-tested.
- Security boundary: `security` (time-bound, fail-closed permission grants).
- Compatibility boundary: `compatibility/wayland`, `compatibility/xwayland`.
- Cross-cutting: `session` (login/lock state machine), `diagnostics`
  (frame-time/input-latency telemetry, fail-closed debug gate),
  `configuration` (serializable `DisplayConfig`).
- Real integration with the sibling `SHER-Input` repo (`sher_input_core`),
  replacing an earlier temporary bridge to `SHER-Kernel`'s `input_driver`.
- CI workflow (`cargo fmt`, `cargo build`, `cargo test`, `cargo clippy -D
  warnings`) with sibling-repo checkouts.
- Relicensed from a proprietary license to Apache-2.0.

### Known incomplete / not started
See [ROADMAP.md](ROADMAP.md) for the authoritative, phase-by-phase status
and [ROADMAP_HONEST.md](ROADMAP_HONEST.md) for the technical-debt and
tooling-gap inventory. Headline gaps: no GPU composition wiring (Phase 3),
no native display protocol, no clipboard/drag-and-drop/screenshot/
recording/animation/headless implementation (manifests/directories don't
exist yet, not even stubs, except `clipboard`'s manifest-only crate), no
Aurora integration.

### Fixed
- Corrected several documentation claims found inaccurate on
  re-verification (stub-crate claims, clipboard `src/` claim, Aurora
  integration framing) — see git log for details; each was a docs-only
  correction, not a behavior change.
