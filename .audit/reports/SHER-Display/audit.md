# Audit Report: SHER-Display

**Audit Date:** 2026-10-26  
**Repository Path:** `/Users/georgimullassery/Desktop/Projects/SHER-Display`  
**Audit Type:** Read-only systems design audit  
**Version:** 0.1.0 (workspace)  
**Branch:** `main`  
**Source Size:** ~3,797 Rust LOC; 15 workspace members; 69 tests

## Executive Summary

SHER-Display is a pre-alpha native display/compositor model: scene graph, surfaces, window/workspace state, per-output frame scheduling, focus-aware input routing, cursor negotiation, session state, diagnostics, and a time-bound permission boundary. The implemented crates are real, testable models, not a renamed Wayland shell. The clipboard crate is now a real implementation with permission checks and bounded text history, which makes much of the README’s “manifest-only stub” language stale.

The central architectural gap is identity. The permission boundary is an in-process policy store keyed by a caller-supplied `app_id` string; nothing binds that string to a process, socket peer, or portal token. Any code in the compositor process can claim another app’s ID, grant itself a permission, or pass a favorable `now`. That is acceptable only while the layer is treated as a testable policy model, not a real security boundary. Clipboard payload/history retention also lacks byte/TTL limits, and documentation still understates the workspace by one crate and seven tests.

## Purpose and Positioning

- **What it is:** a Rust native compositor/window-management model with scene, surfaces, windows, workspaces, outputs, input routing, cursor, session, diagnostics, clipboard, and a permission API.
- **Who it is for:** the SHER OS stack and developers studying a structured, AI-addressable compositor model.
- **What it is not:** a usable desktop, a Wayland protocol implementation, a DRM/evdev compositor, a screenshot/recording/headless product, or a hardened multi-process security boundary.
- **Differentiator:** per-output frame scheduling and a capability-style permission model expressed as headless, testable Rust models.

## Architecture

```text
Application / agent / session
        |
        v
session ── diagnostics ── configuration
        |
        v
windows ── workspaces ── outputs ── cursor
        |
        v
scene ── surfaces ── compositor (damage + FrameScheduler)
        |
        +--> input (focus routing over SHER-Input)
        +--> security (time-bound grants)
        +--> clipboard (ClipboardAccess + text history)
        +--> compatibility/wayland, compatibility/xwayland
```

### Frame path

```text
per-output refresh interval
  -> FrameScheduler deadline
  -> compositor poll/tick
  -> scene traversal/damage
  -> output scheduling model
  -> (real vblank/presentation feedback: not wired)
```

### Input and privilege path

```text
SHER-Input event
  -> sher_display_input focus/capture routing
  -> scene window/surface
  -> security.check(app_id, Permission, now)
  -> privileged operation (capture/injection/clipboard/etc.)
```

## Repository Layout

```text
compositor/                 damage tracking + per-output frame scheduler
scene/                      z-ordered scene graph
surfaces/                   application surface lifecycle
windows/                    window state/layout/snap
workspaces/                 virtual workspaces
outputs/                    output model and refresh metadata
input/                      focus-aware routing over SHER-Input
cursor/                     hardware/software cursor negotiation
security/                   permissions, tiers, grants, expiry
clipboard/                  owner model, formats, permissioned history
compatibility/wayland/      Wayland compatibility seam
compatibility/xwayland/     XWayland compatibility seam
session/                    session state machine
diagnostics/                diagnostics model
configuration/              configuration model
docs/                       architecture, roadmaps, internal audit
.github/workflows/          CI + audit
```

## Technology Stack and Dependencies

- **Rust:** 2021 edition; Tokio, serde, uuid, tracing, thiserror/anyhow.
- **Internal siblings:** SHER-Kernel (`common`, `objectmodel`, `gpu_driver`, `wayland_server`), SHER-Graphics (`graphics_api`, `gpu_abstraction`, `graphics_runtime`, `graphics_compat`), SHER-Input (`core`, `test-support`).
- **Path casing:** siblings are referenced as `../SHER-KERNEL`, `../SHER-Graphics`, `../SHER-INPUT`; the casing is documented and matters on ext4/Linux.
- **Testing:** Rust unit/integration tests; mock input and graphics seams.
- **CI:** fmt, build, test, clippy, cargo audit.

## Public API

- `sher_display_compositor`: damage tracking and `FrameScheduler`.
- `sher_display_scene`: scene graph and hit-testing.
- `sher_display_surfaces`: surface lifecycle.
- `sher_display_windows`: window state, layout, snapping.
- `sher_display_workspaces`: workspace model.
- `sher_display_outputs`: output/refresh model.
- `sher_display_input`: focus-aware input routing and capture over SHER-Input.
- `sher_display_cursor`: cursor negotiation.
- `sher_display_security`: `Permission`, `PermissionTier`, `Grant`, `PermissionManager`.
- `sher_display_clipboard`: `Clipboard`, `ClipboardOffer`, `ClipboardContent`, history entries.
- `sher_display_compat_wayland` / `xwayland`: compatibility seams.
- `sher_display_session`, `sher_display_diagnostics`, `sher_display_configuration`.

## Design Patterns

- **Model-view separation:** compositor/window/session logic is headless and testable.
- **Per-output scheduling:** each output carries its own interval/deadline/tick rather than a global frame clock.
- **Capability-style policy:** every privileged operation is expressed as a permission + tier + expiry.
- **Fail-closed checks:** absent/expired grants return false; `check` does not mutate state.
- **Explicit ownership:** clipboard has one owner and owner-only clear.
- **Compatibility seam:** Wayland/XWayland are edge adapters, not the native model.
- **Honest scaffolding:** unimplemented features are excluded from workspace members rather than shipped as empty crates.

## Data and Storage

- In-memory scene/window/workspace/session state.
- Permission grants in an in-memory `HashMap<(String, Permission), Grant>`.
- Clipboard current offer plus `VecDeque<ClipboardHistoryEntry>` bounded by entry count.
- Clipboard history retains text formats only; binary formats remain in the current offer but are not retained.
- No database, durable session store, audit log, or credential store.
- No encryption at rest; no clipboard TTL or byte-size retention policy.
- `Grant.expires_at` is `u64` based on caller-supplied time.

## Concurrency and Performance

- The workspace is largely synchronous model code.
- `FrameScheduler` is unit-tested headlessly; compositor polling is a model, not a real-vblank render loop.
- `PermissionManager` is `HashMap`-based and not internally synchronized; use from one thread or wrap externally.
- Clipboard is `&mut self`/`&self` and not internally synchronized.
- Input routing consumes SHER-Input's broadcast stream; slow consumers must handle lag/loss.
- No benchmarks for scene traversal, damage tracking, input routing, or multi-output scheduling.
- Real per-output rendering threads/timers are explicitly not implemented.

## Security and Privacy

### Strengths

- Explicit permission enum covering capture, recording, injection, clipboard, inspection, shortcuts, display configuration, remote display, and accessibility.
- Grants are time-bound; expired grants are treated as absent.
- `check` is read-only and fail-closed.
- Clipboard reads require owner identity or `ClipboardAccess`; history always requires the permission.
- Clipboard owner-only clear.
- Coordinates/permission failures produce explicit security errors.
- Path-casing hazard for Linux CI is documented.
- No network service or credential handling in this repo.

### Gaps

- **Caller-supplied identity:** `app_id` is an arbitrary string. There is no binding to PID, socket peer credentials, portal token, or executable identity. Any in-process component can impersonate another app or grant itself.
- **Caller-supplied time:** `now` is a parameter. A privileged caller can pass a small value to keep an expired grant valid; a compromised caller is already inside the boundary.
- **No consent/UI layer:** `PermissionManager::grant` accepts `Critical` and every permission directly. A real desktop needs an out-of-process consent prompt.
- **No audit trail:** grants, denials, and clipboard reads are not logged as security events in this crate.
- **Clipboard retention:** current offers can contain arbitrarily large byte payloads; history is count-bounded but text entries have no byte/TTL cap. Sensitive clipboard text can persist for the process lifetime.
- **No MIME allowlist/size cap:** `set_offer` accepts any non-empty vector of formats.
- **No inter-process boundary:** this is an in-process policy model, not an OS sandbox.
- **No permission revocation propagation events:** revoke changes the map; consumers must re-check.

## Reliability and Observability

- Permission denial is explicit; security errors carry app/permission context.
- Expired grants are treated as absent rather than reported specially.
- `prune_expired` exists but is not called implicitly.
- Session state machine has explicit transitions and tests.
- Diagnostics model exists, but no structured metrics/tracing/health endpoint was reviewed.
- No audit log for security decisions.
- No panic boundary around compositor/input callbacks.
- No recovery path for a poisoned lock because there are no internal locks in the security/clipboard models.

## Testing, CI, and Quality

- 69 static test definitions; README says 62 and 14 crates.
- Tests cover compositor/frame scheduling, scene, windows, workspaces, input isolation, security grants/expiry, clipboard permission/ownership/history, session, diagnostics, and configuration.
- CI runs fmt, build, test, clippy, and cargo audit.
- No tests for app-identity spoofing (because identity does not exist), clipboard payload size, history TTL, or concurrent permission mutation.
- No real Wayland/XWayland protocol tests; compatibility crates are seams.
- No real DRM/KMS or real vblank hardware validation.
- No fuzzing or coverage gate.

## Documentation, Packaging, and Release

- Workspace version is 0.1.0; no tagged releases; pre-alpha.
- README is stale:
  - says 14 crates and 62 tests; actual workspace has 15 members and 69 tests;
  - says clipboard is a manifest-only stub with no `src/` and is not a workspace member; it is implemented and included.
- `docs/internal/AUDIT_FINDINGS.md` also says 14 crates/56 tests and clipboard as a stub.
- README accurately discloses that per-output VSync is a scheduling model, not a real vblank/presentation path.
- Cargo path-casing warning is accurate and valuable.
- No published package or release provenance.

## Strengths

- Coherent modular decomposition of a compositor into testable crates.
- Per-output scheduling model is a genuine differentiator.
- Permission model is simple, explicit, and fail-closed.
- Clipboard implementation is real, with owner bypass and permissioned history.
- Strong headless testability; no GPU/display hardware required for core logic.
- Explicit documentation of path-casing and VSync limitations.
- Honest “not a usable desktop” positioning.
- CI covers fmt/clippy/test/audit.

## Risks and Gaps

| Risk | Severity | Evidence | Impact |
|---|---|---|---|
| Permission identity is caller-supplied | High | `security/src/lib.rs`: `app_id: &str`; no process binding | Any in-process caller can impersonate or self-grant; not a real security boundary. |
| Caller controls expiry time | Medium | `check(..., now)` / `grant(..., now)` | Wall-clock/expiry bypass if the caller is compromised or buggy. |
| No consent layer for Critical grants | Medium | `PermissionManager::grant` accepts all tiers directly | Privileged operations can be granted without user intent. |
| Clipboard payload/history lacks byte/TTL limits | Medium | `set_offer` accepts arbitrary `Vec<u8>`; history text has no TTL | Memory growth and long-lived sensitive text. |
| No security audit trail | Medium | No logging in permission/clipboard paths | Denials/grants/reads are not investigable. |
| README/internal docs undercount workspace/tests | Low/Medium | 14/62/56 vs 15/69; clipboard stub claim | Misleads users and CI expectations. |
| No real vblank/presentation feedback | Medium (known) | README discloses model-only VSync | Real multi-output pacing is unvalidated. |
| Compatibility crates are seams, not protocol implementations | Medium (known) | Wayland/XWayland crates | Not usable as a Wayland compositor. |
| No fuzzing/coverage gate | Low | CI review | Edge cases in scene/input models are unproven. |

## Prioritized Recommendations

### P0 — identity and consent truth

1. Bind `app_id` to a real principal: socket peer credentials, portal token, process start time + PID, or an out-of-process broker. Do not present string-keyed grants as a security boundary.
2. Move time to a trusted monotonic source inside the security boundary, or at minimum bind `now` to a clock the caller cannot arbitrarily rewind.
3. Require an out-of-process consent step for `PermissionTier::Critical` and high-impact permissions; keep the in-process manager as policy state, not consent.

### P1 — clipboard and auditability

4. Add max payload bytes, max total history bytes, MIME allowlists, and TTL/expiry to clipboard offers/history.
5. Emit structured audit events for grant/revoke/check-denial/clipboard-read, with app identity and permission.
6. Make `PermissionManager` explicitly single-threaded or wrap it in a synchronized service; document the concurrency contract.

### P2 — docs and verification

7. Update README and internal docs to 15 crates/69 tests and the real clipboard implementation.
8. Add tests for clipboard size limits, history TTL, permission mutation under concurrency, and audit events.
9. Add scene/input fuzz or property tests and a coverage gate.
10. Keep the VSync limitation prominent; add a hardware-backed follow-up plan before claiming real per-output pacing.

## Audit Limitations

- No real Wayland/XWayland, DRM/KMS, input device, or GPU session was run.
- Security findings are based on static review of the in-process model, not a multi-process threat model exercise.
- Clipboard limits were verified by reading the implementation, not by memory profiling.
- Test counts are static definitions, not a verified pass count.
- Internal docs may reflect older snapshots; only the cited claims were checked.
