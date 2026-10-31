# WP-100 progress — exceptions

Branch `wp/100-codegen-eh`, worktree `kiln/.worktrees/wp-100`.

## Step 1 — sema / AST / parser (in progress)
- AST: `CXXTryStmt` (block + handlers) / `CXXCatchStmt` (optional
  exception var + block) builders, accessors, dump; `opaque_value_of`.
- Parser: `try` statement, handlers (`catch (...)`, declarator forms),
  function-try-blocks (`f() try {...} catch ...`, ctor with
  mem-initializers), `expected catch`.
- Sema (`try_stmt.rs`): exception-declaration checks
  ([except.handle]/1-3: rvalue ref, incomplete, abstract, decay), class
  handler copy-initialized from an OpaqueValueExpr + destructor check,
  early catch-all error, `-Wexceptions` "will be caught by earlier
  handler" (duplicate and public base) + note, try barrier for implicit
  move of thrown locals ([class.copy.elision]/3), falloff analysis for
  try/throw, return in ctor function-try-block handler.
- xtask clang-ast printer: CXXTryStmt/CXXCatchStmt kept; unnamed VarDecl.
- Test: `tests/conformance/ast/except.handle/try-catch.cpp` (golden exact).

## Next
- Diag conformance tests in `tests/conformance/except.handle`, `except.pre`.
- Step 2: Oven `try` in constexpr functions.
- Step 3: codegen EH (`kiln_codegen/src/eh.rs`, invoke/landingpad/resume).
- Step 4: run tests, spec updates, full CI.
