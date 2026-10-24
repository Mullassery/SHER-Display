# Competitive intelligence in roadmap and upgrade work

**Policy date:** 2026-10-19

Competitive findings are an input to product decisions, not a one-off report.
Every pending roadmap item, new feature, and routine dependency upgrade should
check the repository's [`AUDIT_FINDINGS.md`](AUDIT_FINDINGS.md) before work
starts, and record the decision in the PR or changelog.

## Required check before building

1. **Read `AUDIT_FINDINGS.md`.** Find the relevant competitor row and the
   ranked next actions. If the finding is stale, update it with a dated source.
2. **Name the competitor and the user problem.** A feature is not justified by
   "competitor has it". State the workflow it unlocks, who needs it, and how
   often. Mark unverified demand as a hypothesis.
3. **Prefer reuse over new code.** Check whether an existing Mullassery repo
   already provides the capability (see `BUNDLES.md` in the profile repo and
   the bundle role file in this repo, when present). Typical reuse:
   - crawling/fetching/robots/SSRF → **MudFish**
   - architecture maps / PR diffs → **CodeAtlas**
   - data-quality contracts / validation → **StatGuardian**
   - token counting / cost tables → **PyTokenCalc**
   - dependency graph / advisories / SBOM → **PyDeepMesh**
   - segmentation / CLV / churn → **ClusterAudienceKit**
   - activation / lineage / PII → **PyReverseETL**
   - fleet state sync → **SyncFleet** (only after conflict resolution)
4. **Choose one disposition:** build, integrate, defer, or reject. Write it
   down. A rejection with a reason is a successful outcome.
5. **For routine upgrades**, check whether the upgrade closes a known
   competitor gap or breaks a differentiated capability. Run the tests that
   cover the affected path; do not merge a green build that silently changes a
   user-visible contract.

## PR / changelog checklist

- [ ] Competitor and source URL/date recorded (or "no direct competitor").
- [ ] User impact and frequency stated; unverified demand labelled.
- [ ] Reuse check done against sibling repos/bundles.
- [ ] Failure mode and rollback considered (especially auth, PII, cost).
- [ ] Docs/roadmap status updated honestly.
- [ ] Tests or a manual verification command included.

## Review cadence

- Refresh the competitor row when a release changes the capability.
- Re-check the full `AUDIT_FINDINGS.md` at least quarterly and before a major
  release.
- If a finding is wrong or stale, correct it in the same PR that relies on it.

## Non-goals

- Copying features for parity.
- Adding a dependency just because a competitor uses it.
- Treating star counts or download counts as proof of user need.

## OSS gap analysis — 2026-10-24

**What matters most here:** a runnable compositor and client protocol. Libraries alone cannot start a session; ship `sher_display_protocol`, a backend, and DRM/KMS composition before comparing with Smithay or Sway.

Scope: assessed SHER-Display's 14-crate compositor/window-management prototype against Smithay (Rust compositor library), cosmic-comp (production Rust Wayland compositor) and Sway (mature wlroots compositor) on runnability, protocol coverage, backends, buffer semantics and desktop features.

### Comparable OSS projects
| Project | GitHub | License | Last activity | Why it is the comparison |
| --- | --- | --- | --- | --- |
| Smithay | https://github.com/Smithay/smithay | MIT | 0.7.0, 2026-10-17 | Closest peer: a Rust library of Wayland compositor building blocks, not a finished desktop. |
| cosmic-comp | https://github.com/pop-os/cosmic-comp | GPL-3.0 | COSMIC Epoch 1.4.0, 2026-07-22 | Rust Wayland compositor used as a real desktop; the production end-state SHER-Display gestures at. |
| Sway | https://github.com/swaywm/sway | MIT | 1.12, 2026-05-25 | Mature, widely deployed wlroots compositor; the baseline for protocol and feature coverage. |

### Capability comparison
| Capability | Ours | Smithay | cosmic-comp | Sway | Gap |
| --- | --- | --- | --- | --- | --- |
| Runnable session/compositor | No | Partial | Yes | Yes | Fundamental |
| Native client protocol | No | Yes | Yes | Yes | Fundamental |
| Wayland protocol coverage | Partial | Yes | Yes | Yes | Large |
| DRM/KMS backend | No | Yes | Yes | Yes | Large |
| GPU composition | No | Yes | Yes | Yes | Large |
| Multi-output / mixed refresh | Yes | Yes | Yes | Yes | None |
| Window management model | Yes | Partial | Yes | Yes | Small |
| Workspaces | Yes | Partial | Yes | Yes | Small |
| Focus-aware input routing | Yes | Yes | Yes | Yes | None |
| XWayland | Partial | Yes | Yes | Yes | Large |
| Clipboard / drag-and-drop | No | Partial | Yes | Yes | Large |
| Screenshot / recording | No | Partial | Yes | Partial | Large |
| Accessibility | No | No | Partial | Partial | Large |
| Time-bound permission grants | Yes | No | No | No | Ours ahead |
| AI/agent window API | No | No | No | No | Parity |

Ours models policy but is not a runnable compositor: no binary, no DRM/KMS or nested backend, no `sher_display_protocol` (ROADMAP.md Phase 1 lists all three as not started). The Wayland seam handles surface create/attach/commit/destroy only; Smithay supports core Wayland plus wlroots/KDE extensions. Our XWayland support is an XID-to-surface-id table, not a running XWayland client.

### Gaps that matter (ranked)
1. **We lack a runnable compositor and client protocol** — `cargo build` produces libraries only; ROADMAP.md confirms no display-server lifecycle, `sher_display_protocol` or backend. Users cannot start a session or connect a client; effort L.
2. **No DRM/KMS or GPU-composition path** — `compositor` stops at `FrameReport`; Phase 3 GPU wiring is not started. Every comparison implements this; effort L.
3. **Wayland coverage is a thin seam, not an implementation** — only client-connect/surface lifecycle mapping, with no xdg-shell, layer-shell, dmabuf or sync protocols. Real applications will not run; effort L.
4. **Buffer release/fence semantics are missing** — `SurfaceState.buffer_id` has no release notification or fence (ROADMAP.md Phase 1), blocking safe zero-copy/GPU composition. "We have a weaker version" of buffer management; effort M.
5. **Baseline desktop features are absent** — clipboard is a manifest-only stub; drag-and-drop, screenshot, recording, headless and accessibility are unscaffolded. This is what users notice first; effort M each.
6. **Security is policy, not enforcement** — time-bound grants exist, but there is no protocol-level or OS-level enforcement behind them; effort M/L.

### Where we are differentiated
- Independent per-output frame scheduling with per-output refresh intervals and deadlines is a first-class, headless-testable model — stronger than treating one global frame clock as the compositor.
- Input isolation is enforced by construction: `RoutedEvent::Focused` cannot name an arbitrary surface, so an unfocused client has no code path to another client's keystrokes. In Sway/wlroots this is compositor policy, not a type-level invariant.
- Time-bound, fail-closed permission grants for capture, recording, injection and clipboard have no equivalent in the three comparators.
- An explicit session state machine that rejects illegal transitions is cleaner than the implicit session handling typical of these projects.

### Disposition
- **Build:** `sher_display_protocol`, a backend trait plus a Linux DRM/evdev backend, buffer release/fence handling, and scene-graph hit-testing. These are the minimum for any real session.
- **Integrate/reuse:** use `wayland-rs`, `wayland-protocols` and Smithay's protocol handlers rather than reimplementing Wayland; reuse wlroots protocol XML and dmabuf semantics; consume SHER-Input/libinput instead of writing device handling.
- **Defer/reject:** full Wayland/X11 desktop parity, owning a GPU driver, and reimplementing wlroots; keep the native model as the differentiator, not a replacement protocol stack.

### Sources
- https://github.com/Smithay/smithay (accessed 2026-10-24)
- https://docs.rs/crate/smithay/latest (0.7.0, accessed 2026-10-24)
- https://github.com/pop-os/cosmic-comp (accessed 2026-10-24)
- https://github.com/swaywm/sway (accessed 2026-10-24)
- https://github.com/swaywm/sway/releases/tag/1.12 (accessed 2026-10-24)
- Repo files inspected: `README.md`, `COMPETITIVE_INTELLIGENCE.md`, `AUDIT_FINDINGS.md`, `ROADMAP.md`, `ROADMAP_HONEST.md`, `Cargo.toml`, `docs/architecture/README.md`
