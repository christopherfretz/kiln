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
- **Never `git stash` in a worktree.** `refs/stash` belongs to the repository,
  not the worktree: two agents stashing concurrently pop each other's changes
  (WP-213/WP-215, 2026-11-01). Commit work in progress, or use `git diff >
  x.patch` / `git apply` instead.

1. Work in the git worktree the supervisor created for you at
   `.worktrees/wp-<id>` (branch `wp/<id>-<slug>`), addressing it by absolute
   path in every command; never on `main`, never in another worktree.
   (The Agent tool's built-in worktree isolation is broken in this
   environment: subagents lose their Bash context. The supervisor runs
   `git worktree add .worktrees/wp-<id> -b wp/<id>-<slug>` before spawning.)
2. Read `README.md`, `docs/ARCHITECTURE.md`, the `spec/` entry, and the
   crate's `lib.rs` docs before writing code.
3. `#![forbid(unsafe_code)]` stays. The toolchain is pinned to Rust 1.95
   (`rust-toolchain.toml`, `rust-version`; a downstream build has nothing
   newer): no std API, language feature or clippy lint name stabilized after
   1.95. The gate is `cargo xtask ci`: fmt,
   clippy `-D warnings`, `cargo test`, and the corpus under both clang 19
   (the self-check) and kiln (no FAIL, XPASS or ERROR; spec/harness.md
   "CI"). A kiln gap your WP does not close is marked `xfail(kiln):
   <reason>` in the test, not left failing. Since 2026-11-01 the full gate
   runs off-box: every handback becomes a pull request and GitHub's merge
   queue runs `ci` (docs/VALIDATION.md "GitHub Actions") before main moves.
   Locally, hand back once `cargo xtask ci --quick` (no clang self-check),
   the corpus dirs you touched, fmt and clippy are clean and origin/main is
   merged in. Implementers never push to origin or merge to main; the
   supervisor pushes the branch, opens the PR and queues it.
4. Do not change a public interface outside your WP without recording the
   change in the spec and flagging it in the handback. Other agents may be
   building against it concurrently.
5. Diagnostics, not panics, for anything derivable from user input. Internal
   invariants use `unreachable!`/`debug_assert!` with a message.
6. Every new behavior ships with a test. Every test names its standard
   section. Every spec change ships with its tests.
7. No new third-party dependency without approval in the WP.
8. Hand back with: the branch head, a summary, the test evidence (commands +
   results, quick-CI log path), open questions, and concrete follow-ups
   (file:line) for a BACKLOG row.
9. New diagnostic ids and warning groups go in kiln_diag's area files
   (`crates/kiln_diag/src/ids/<prefix>*.rs`, `src/groups/*.rs`; spec/diag.md
   §2.4), sorted by name, never appended to one shared table end. Either
   write the row in place or append it to the `diag_id_table! { }` /
   `diag_group_table! { }` tail at the end of `ids.rs`/`groups.rs` and run
   `cargo xtask diag-split`; `cargo xtask ci` rejects a non-empty tail or a
   misplaced row. When merging origin/main conflicts in those files
   (including a branch from before the split), run
   `cargo xtask diag-split --replay` during the merge: it takes main's
   tables and re-applies your branch's added, changed and removed rows by
   name, then `git add crates/kiln_diag && git commit`. xtask links
   kiln_diag, so it cannot build while any kiln_diag file (tables or area
   files such as `groups/umbrellas.rs`) holds conflict markers; first reset
   exactly the conflicted kiln_diag files to main's side, then replay:
   `git diff --name-only --diff-filter=U -- crates/kiln_diag | xargs git
   checkout MERGE_HEAD --` (your rows are replayed from `HEAD`, nothing is
   lost; code changes outside the tables in those files must be re-applied
   by hand, the tool warns).
10. A WP's "Landed" spec section goes in its own file,
   `spec/landed/<spec>/WP-NNN.md` (first line `# WP-NNN — <title>`, the
   body shaped as the spec's existing Landed sections), never appended
   to the end of `spec/<spec>.md`: two open PRs appending to the same
   spec conflict, and each landing then forces a re-merge and a full CI
   run of the others (spec/README.md "Landed sections"). Do not number
   it (`§7x`, `§8.NN`); existing sections stay where they are. `cargo
   xtask ci` fails on a file there without a `| WP-NNN |` row in
   `docs/BACKLOG.md`, so the WP must have its row (the supervisor adds
   it when briefing).

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
