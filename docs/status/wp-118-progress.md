# WP-118 progress — codegen follow-ups from WP-100

Branch `wp/118-codegen-new-cleanups`, worktree `kiln/.worktrees/wp-118`.

## 1. Array-new / member array ctor loops (in progress)
- sema: `new K[2][3]` (and `()`) builds a `CXXConstructExpr` of the
  array type (was: no initializer / ImplicitValueInitExpr);
  mem-initializer `arr()` for an array of classes likewise.
- codegen: per-element zeroing in the array-new ctor loop (was an error);
  trivial-ctor zeroing via memset; no pre-zeroing memset of arrays of
  classes with a user-declared ctor (`CheckAggExprForMemSetUse`).
