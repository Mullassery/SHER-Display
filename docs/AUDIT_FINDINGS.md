# Audit Findings: SHER-Display

**Audit Date:** 2026-10-26  
**Version:** 0.1.0  
**Branch:** `main`  
**Full report:** [`.audit/reports/SHER-Display/audit.md`](../.audit/reports/SHER-Display/audit.md)

## Scope

Read-only review of the 15-crate compositor/window stack, security and clipboard crates, compatibility seams, tests, CI, and documentation. No real Wayland/X11/DRM/GPU session was run.

## Findings

| ID | Severity | Finding | Evidence | Recommendation |
|---|---|---|---|---|
| SD-01 | High | Permission identity is a caller-supplied `app_id` string with no process binding. | `security/src/lib.rs`: `check(app_id, ...)`, `grant(app_id, ...)`. | Bind identity to peer credentials/portal token or an out-of-process broker. |
| SD-02 | Medium | Grant expiry trusts caller-supplied `now`. | `check(..., now)` / `grant(..., now)`. | Use a trusted monotonic clock or a non-rewindable time source. |
| SD-03 | Medium | Critical grants need no consent/UI step. | `PermissionManager::grant` accepts all tiers directly. | Add out-of-process consent for high-impact permissions. |
| SD-04 | Medium | Clipboard payloads/history lack byte and TTL limits. | `set_offer` accepts arbitrary `Vec<u8>`; history text has no TTL. | Add max bytes, MIME allowlist, and retention TTL. |
| SD-05 | Medium | No security audit trail. | No grant/denial/read logging in security/clipboard. | Emit structured audit events. |
| SD-06 | Low/Medium | README/internal docs undercount crates and tests. | 14/62/56 claims vs 15 members/69 tests; clipboard stub claim. | Update docs and CI expectations. |
| SD-07 | Medium (known) | VSync is a scheduling model, not real vblank/presentation feedback. | README known issues. | Keep limitation prominent; add hardware-backed pacing later. |
| SD-08 | Medium (known) | Wayland/XWayland crates are seams, not protocol implementations. | Compatibility crates. | Do not present as a usable compositor yet. |
| SD-09 | Low | No fuzzing/coverage gate for scene/input models. | CI review. | Add property/fuzz tests and a coverage gate. |

## Positive Controls

- Explicit permission enum and time-bound grants.
- `check` is read-only and fail-closed.
- Clipboard owner-only clear and permissioned reads/history.
- Text-only retained history; binary formats are not retained.
- Headless-testable compositor/scene/window/session models.
- Per-output frame scheduler is a genuine design differentiator.
- Documented case-sensitive sibling-path requirement.
- CI runs fmt, build, test, clippy, and cargo audit.

## Audit Limitations

- No multi-process security exercise.
- No real Wayland/X11/DRM/GPU run.
- Clipboard size/retention issues found statically, not profiled.
- Test counts are definitions, not a verified pass count.
- Internal docs may be older than the current tree.
