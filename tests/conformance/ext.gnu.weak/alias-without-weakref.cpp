// GCC "Common Function Attributes", alias: `alias("target")` alone makes
// the declaration another name for `target` (clang emits a GlobalAlias).
// kiln's codegen handles the attribute only as a `weakref` target
// (BACKLOG WP-635), so sema reports it ignored rather than let the
// reference go to an undefined symbol.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang emits the alias; kiln does not yet
//@ diag: 11:36 warning "'alias' attribute ignored"

extern int g19(int) __attribute__((alias("q")));
