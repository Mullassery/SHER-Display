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
- Independent per-output frame scheduling (C11) in `compositor`: a pure
  `FrameScheduler` gives every output its own refresh interval, next
  deadline, and tick counter. The compositor's `tick` polls all outputs
  independently, so mixed-refresh layouts (e.g. 60 Hz + 144 Hz) are not
  paced by a single global clock, and a late poll coalesces missed periods
  instead of emitting a frame burst. Changing one output's refresh rate or
  unregistering it leaves other outputs' deadlines untouched. Covered by
  mixed-rate deadline/coupling tests plus a 10-second headless 60/144 Hz
  simulation (workspace now 62 tests).
- Security boundary: `security` (time-bound, fail-closed permission grants).
- Compatibility boundary: `compatibility/wayland`, `compatibility/xwayland`.
- Cross-cutting: `session` (login/lock state machine), `diagnostics`
  (frame-time/input-latency telemetry, fail-closed debug gate),
  `configuration` (serializable `DisplayConfig`).
- Real integration with the sibling `SHER-INPUT` repo (`sher_input_core`),
  replacing an earlier temporary bridge to `SHER-KERNEL`'s `input_driver`.
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

Specifically for C11/per-output VSync: scheduling is implemented and
headlessly tested, but it is not yet driven by real vblank or
presentation-completion feedback from SHER-Graphics, and the compositor
still polls from one loop rather than owning per-output OS timer/render
threads. Whether one slow output can delay another's actual *rendering* on
real hardware remains unvalidated (ROADMAP Phase 3).

### Fixed
- Corrected several documentation claims found inaccurate on
  re-verification (stub-crate claims, clipboard `src/` claim, Aurora
  integration framing) — see git log for details; each was a docs-only
  correction, not a behavior change.
- Hardened the two unwrap-after-invariant-check sites flagged in
  `ROADMAP_HONEST.md`: `windows/src/lib.rs:126` (`activate()`) and
  `compositor/src/lib.rs:104` and `:113` (`tick()`). Replaced bare
  `.unwrap()` with `.expect("<invariant, spelled out>")` so a future
  refactor that breaks the invariant panics with an explanation instead of
  a bare "called `Option::unwrap()` on a `None` value". No behavior change
  for any currently-passing path (still panics only if the invariant is
  ever actually violated); confirmed via full workspace test/clippy/fmt
  pass, 56/56 tests still green.
- Ran `cargo audit` for real this pass (network was available, unlike the
  prior pass): 75 crate dependencies scanned against 1258 RustSec
  advisories, zero vulnerabilities found. See `ROADMAP_HONEST.md` for the
  updated status.
