# Stability and versioning

kiln follows [semantic versioning](https://semver.org/). Its version is
the workspace version in `Cargo.toml` (spec/driver.md "Version"): kiln
prints it in `--version`, defines it as `__KILN_MAJOR__`,
`__KILN_MINOR__` and `__KILN_PATCH__`, and puts it in the release archive
names. This document says what a version number promises: what may not
change incompatibly before the next major version, and what may change
in any release.

Before 1.0 (`0.y.z`) the rules below are the target, not a promise: a
minor version may break anything here, and the release notes say what
broke. From 1.0 on they bind.

## Covered: stable within a major version

A minor or patch release does not remove, rename or change the meaning
of any of these. It may add new ones.

- **Driver flags documented in [spec/driver.md](../spec/driver.md).** A
  flag that spec/driver.md documents keeps its spelling, its arguments
  and its effect. Flags kiln accepts only for clang compatibility keep
  matching clang's meaning. A flag kiln accepts but spec/driver.md does
  not document carries no promise.
- **Exit status.** 0 for success, 1 when an error was reported, and 70
  for an internal compiler error (spec/driver.md "Output, diagnostics,
  exit status").
- **Diagnostic identifiers.** The diagnostic ids (`sema_...`, `pp_...`:
  the stable names in the JSON output and in `-verify` expectations;
  [spec/diag.md](../spec/diag.md) §2.1), the warning groups and the
  `-W<group>`, `-Wno-<group>`, `-Werror=<group>` names that select them
  (§3, §4) are never renamed or reused. An id may be retired; a retired
  `-W` name is still accepted and then selects nothing. A warning may
  move into a group, or a new group, under its old `-W` names.
- **Predefined macros.** `__KILN__`, `__KILN_MAJOR__`, `__KILN_MINOR__`
  and `__KILN_PATCH__`, the clang compatibility macros (`__clang__`,
  `__clang_major__` and the others spec/driver.md "Predefines" lists) and
  the `__cpp_*` feature-test macros keep their meaning. A `__cpp_*` value
  only rises, when a feature is completed or a later paper is
  implemented, as the standard intends.
- **ABI and name mangling.** Object code from two releases with the same
  major version links and runs together. kiln follows the Itanium C++ ABI
  as clang 19 does for the same target ([spec/abi.md](../spec/abi.md)):
  layout, calling convention and mangled names match clang's, and a
  release that finds it differs from clang treats that as a bug and says
  so in its notes. Names whose mangling clang does not define yet
  (reflection, `-fkiln-reflection-mangling`) are covered from the release
  that documents them as final in spec/abi.md.
- **The install layout** (spec/driver.md "Installation layout"):
  `bin/kiln`, `bin/kiln++`, `lib/kiln/<major>/` and how kiln finds its
  resource directory.

## Not covered: may change in any release

- **Binary module interfaces (BMI files) are explicitly unstable.** A
  BMI (precompiled module interface, [spec/modules.md](../spec/modules.md))
  is meant to be read only by the kiln build that wrote it; its format
  may change in any release, patch releases included. Build systems must
  not ship BMIs or cache them across kiln versions; rebuild them from the
  interface sources.
- **`-fkiln-*` flags are experimental.** The kiln-specific extensions
  (`-fkiln-stack-size`, `-fkiln-instantiation-report`,
  `-fkiln-memory-report`, `-fkiln-reflection-mangling`,
  `-fkiln-source-path-style`, `-fkiln-outline-expansions`,
  `-fkiln-dump-cfg`, `-fkiln-reproducible-time` and the others), the
  `__kiln_*` builtins and the formats of the reports they write may
  change or go away in a minor release. One graduates to a covered flag
  when spec/driver.md says so.
- **Diagnostic wording.** The text of messages, notes and fix-it hints,
  their order, and the caret and source-range rendering. kiln aims to
  match clang's wording ([spec/diag.md](../spec/diag.md) §2.5) and
  improves it freely; match on ids (`-fdiagnostics-format=json`), not on
  text.
- **LLVM IR text.** The IR `-S -emit-llvm` writes: value names, metadata,
  instruction order, attributes and which LLVM version's dialect it is.
  Only the behaviour of the compiled program is specified.
- **Performance.** Compile time and memory are tracked and kept within
  budget ([docs/VALIDATION.md](VALIDATION.md)), but a number is not a
  promise. Constant-evaluation limits (`-fconstexpr-steps` and the
  others) keep their meaning; how many steps a given program takes may
  change.
- **The Rust crates' APIs** (below), the test harness (`cargo xtask`) and
  everything under `spec/` that is not one of the covered items.

## Deprecation

Removing or changing a covered item takes a deprecation first:

1. A minor release deprecates it: kiln keeps accepting it and warns
   (`-Wdeprecated` group, or a `drv_` warning for a flag), and the
   release notes and spec/driver.md say what replaces it.
2. It stays at least until the next major version and at least six
   months.
3. The next major version may remove it.

A correction of a conformance bug (kiln accepted a program the standard
forbids, or rejected a valid one, or miscompiled one) is not a breaking
change and needs no deprecation, though the release notes name it.
`-fkiln-*` flags may be removed without a deprecation period; kiln warns
for one release where it can.

## Minimum supported Rust version

kiln builds with Rust **1.95**, pinned in `rust-toolchain.toml` and
`rust-version` in `Cargo.toml`. kiln uses no standard library API or
language feature newer than 1.95. Raising the MSRV is a minor-version
change, announced in the release notes one release ahead; it is not
taken lightly, because some users can only use an older toolchain. A
binary release needs no Rust at all.

## crates.io: not published

**Decision (WP-927): kiln is distributed as a binary (the GitHub release
archives of `cargo xtask dist`) and as source (the repository); its
crates are not published to crates.io.** `publish` stays unset and no
release step publishes; this note records what publishing would take so
the question does not need to be reopened from scratch.

- **What would be published.** Only `kiln_driver` provides something a
  user wants from `cargo install`, the `kiln` binary, but `cargo install`
  cannot put the `kiln++` symlink or the resource tree
  (`lib/kiln/<major>/`) in place, so an installed `kiln` would find no
  builtin headers or predefines unless `KILN_RESOURCE_DIR` pointed at a
  checkout. Publishing it means publishing it and the eleven crates it
  depends on, all twelve workspace crates (`kiln_source`, `kiln_diag`, `kiln_lex`, `kiln_pp`, `kiln_ast`,
  `kiln_parse`, `kiln_sema`, `kiln_oven`, `kiln_abi`, `kiln_llvm`,
  `kiln_codegen`, `kiln_driver`), in dependency order. `xtask` is never
  published.
- **Path dependencies would need versions.** The workspace dependencies
  are `{ path = "crates/..." }` only; crates.io rejects a path dependency
  without a `version`, so each would become `{ path = ..., version =
  "=X.Y.Z" }` and move in lockstep with the workspace version.
- **The resource tree would have to be packaged.** `resource/` (8 MB of
  headers and predefines) lives outside any crate. It would have to move
  into `kiln_driver`'s package (and its `include` list) and be found
  through a crate-relative path, or be embedded in the binary; both
  change the discovery of spec/driver.md "Toolchain".
- **The crates have no stable API.** Every crate's public items are
  internal interfaces between pipeline stages that change in most work
  packages. Publishing them would invite dependents that the next minor
  release breaks, or force semver discipline on internals.

Recommendation: do not publish. Revisit only if a library use (embedding
the Oven, say) has a real user, and then publish that crate alone with an
API designed for it. Reserving the `kiln_*` names on crates.io without
code is a separate, cheap decision for the operator.
