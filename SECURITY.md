# Security Policy

## Current status: pre-alpha, not audited, not hardened

SHER-Display is an early-stage scaffold (14 crates, in-memory / simulated
state, no real hardware or network-facing code paths yet — see README.md's
"What's actually here, not just planned" and "Known limitations" sections).
It has:

- **Never had a third-party security audit.**
- **No fuzzing, no `cargo audit` run in CI yet** (see
  [ROADMAP_HONEST.md](ROADMAP_HONEST.md) for why, and what's tracked to fix
  that).
- **No real attack surface today.** There is no network listener, no
  privilege boundary crossing real processes, and no code that touches real
  hardware — `security/` (permission grants) and `session/` (login/lock
  state machine) are in-memory data structures with unit tests, not
  something an external attacker can currently reach. Do not treat anything
  in this repository as providing real security guarantees yet; the
  `security` crate models an intended *policy* (time-bound permission
  grants, fail-closed expiry), it does not yet enforce anything against real
  system resources.

Do not deploy this project in any context where its security properties
matter until this notice is updated.

## Reporting a vulnerability

If you find a security-relevant bug (a boundary-violation, a
memory-safety issue, a logic bug in `security/` or `session/`'s
state-machine invariants, or anything that would matter once this code is
wired to real hardware), please report it privately rather than opening a
public GitHub issue:

- Email: mullassery@gmail.com
- Include: affected crate/file, a minimal reproduction if possible, and the
  potential impact.

This is a single-maintainer project. There is no dedicated security team, no
bug bounty, and no guaranteed response-time SLA — but reports will be read
and acknowledged.

## Supported versions

There are no tagged releases yet (see [CHANGELOG.md](CHANGELOG.md)). Only
the `main` branch is supported.
