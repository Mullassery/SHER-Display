# Audit findings — 2026-10-19

This file records the repository-specific findings from the portfolio audit
conducted on 2026-10-19. It is documentation only; no code or release artifacts
were changed in this pass unless explicitly noted under "Implementation status".

Sources: the repository's own README/source/workflows, GitHub metadata and
Actions runs, PyPI metadata, and official competitor documentation. Claims are
marked as verified, inferred, or open questions. "Absent" means not found in the
inspected repository/docs, not proof that it cannot exist.

## What it is

Native display server, compositor, and window-management subsystem for the SHER OS family.

## Verified strengths

- 14 crates, 56 tests; compositor/scene/windows/workspaces/input isolation/security grants/Wayland seams; explicit session state machine; fail-closed diagnostics.
- Input isolation by construction is a thoughtful design.

## Verified gaps and user-facing risks

- Not a usable desktop; clipboard is a manifest-only stub; no screenshot/recording/headless/AI APIs.
- Depends on sibling repos via relative paths; no releases; pre-alpha.

## Direct competitors

- Wayland/wlroots, Smithay, X11.

## Ranked next actions

- P3: freeze; no desktop parity work without a user and a hardware/OS plan.

## Do not build

- A usable compositor/desktop; far beyond current scope.

## Evidence gaps / validation needed

- No real applications; no hardware validation.

## Recommended disposition

Freeze as part of the SHER research portfolio.

## Implementation status

No code changes in this pass.
