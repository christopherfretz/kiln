# Working model for agents

The supervisor agent (this repo's resident Claude Code session) plans,
coordinates, reviews and signs off releases. Implementation is done by
subagents working on **work packages**. This file is the contract between
them.

## Work packages

A work package (WP) is a GitHub issue (or, until issues are set up, an entry
in `docs/BACKLOG.md`) with:
- **Scope**: crate/module, the `spec/` section it implements, the standard
  sections, and what is explicitly out of scope.
- **Interface**: the Rust API it must expose (signatures), or the spec entry
  that defines it. Interfaces are fixed before parallel work starts.
- **Tests**: the conformance/unit tests it must add or make pass, by path.
- **Done when**: concrete, checkable, including perf obligations if any.
- **Dependencies**: other WPs that must land first.

WPs are sized so that one subagent finishes in a single session with a
reviewable diff (roughly: one module, one spec section, < ~2k lines).

## Rules for implementers

1. Work in the git worktree the supervisor created for you at
   `.worktrees/wp-<id>` (branch `wp/<id>-<slug>`), addressing it by absolute
   path in every command; never on `main`, never in another worktree.
   (The Agent tool's built-in worktree isolation is broken in this
   environment: subagents lose their Bash context. The supervisor runs
   `git worktree add .worktrees/wp-<id> -b wp/<id>-<slug>` before spawning.)
2. Read `README.md`, `docs/ARCHITECTURE.md`, the `spec/` entry, and the
   crate's `lib.rs` docs before writing code.
3. `#![forbid(unsafe_code)]` stays. `cargo xtask ci` must be green before
   handing back: fmt, clippy `-D warnings`, `cargo test`, and the corpus
   under both clang 19 (the self-check) and kiln (no FAIL, XPASS or ERROR;
   spec/harness.md "CI"). A kiln gap your WP does not close is marked
   `xfail(kiln): <reason>` in the test, not left failing.
   `cargo xtask ci --quick` (no clang self-check) is fine while iterating.
4. Do not change a public interface outside your WP without recording the
   change in the spec and flagging it in the handback. Other agents may be
   building against it concurrently.
5. Diagnostics, not panics, for anything derivable from user input. Internal
   invariants use `unreachable!`/`debug_assert!` with a message.
6. Every new behavior ships with a test. Every test names its standard
   section. Every spec change ships with its tests.
7. No new third-party dependency without approval in the WP.
8. Hand back with: a summary, the test evidence (commands + results), open
   questions, and any spec gaps discovered.

## Review gates (supervisor)

- Spec conformance: does the diff do what the spec says, no more, no less?
- Tests: are the obligated tests present and meaningful (not tautological)?
- Safety and style: forbid(unsafe), clippy, no panics on input.
- Interfaces: no undocumented API drift.
- Perf: baselines updated with justification where numbers moved.
Only the supervisor merges to `main`.

## Parallelism map

Work that can proceed independently at the same time, by milestone, is
listed in `docs/ROADMAP.md`. The structural enablers are: one crate per
stage, specs fixed before implementation, the AST dump format as the
parser/sema contract, and the `SemaBridge` trait as the Oven/sema contract.
