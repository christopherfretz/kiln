# WP-100 progress — exceptions

Branch `wp/100-codegen-eh`, worktree `kiln/.worktrees/wp-100`.

## Step 1 — sema / AST / parser (done)
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
- -Wexceptions throw in a non-throwing function (+ dtor/deallocator
  note), -Winvalid-noreturn falloff; analysis warnings skipped in
  templates (clang skips dependent contexts).
- Tests: `ast/except.handle/try-catch.cpp` (golden exact),
  `except.handle/{handler-order,catch-decl-invalid,catch-all-last,
  ctor-handler-return,return-type-try}.cpp`,
  `except.pre/{try-expected-catch,try-no-exceptions}.cpp`,
  `except.terminate/throw-in-noexcept.cpp`, `dcl.attr.noreturn/falloff.cpp`
  (all verified against clang++-19).

## Step 2 — Oven (done)
- Oven evaluates a try-block's compound statement (handlers are never
  entered: a throw is not a constant expression); test in
  `tests/kiln` constexpr try-block.

## Step 3 — codegen EH (done)
- `eh.rs` (EH scope stack use, invoke dest, landing pads, dispatch,
  resume, terminate scopes, personality, TryMarkNoThrow), `try_catch.rs`
  (catch scopes, dispatch, InitCatchParam, EndCatch, rethrow), throw
  (FreeException), function-try-blocks, partial-construction cleanups
  (bases, members, vbases, delegating, regular and irregular partial
  arrays, aggregate members/bases with deferred deactivation),
  new-expression delete cleanup, array-new constructor loop, bad_cast /
  bad_typeid invoke, IsDestructed element slots.
- Guards instead of silent miscompiles: array cookies, zero-initializing
  array new with constructors.
- Corpus: 10 `tests/codegen/eh_*.cpp`, exact on both triples (whole
  corpus 90 programs, IR exact on both).

## Step 4 — wrap-up (done)
- Run tests: `except.handle/run-throw-catch`,
  `except.ctor/run-unwind-destroy`, `expr.new/run-delete-on-throw`
  (kiln and clang both 0). No existing `xfail(kiln)` test involved
  exceptions, so none flipped.
- Spec: codegen.md §7b, sema.md §5.5/§7.5, ast.md §6, parse.md landed
  note — written with the leftovers.
- Quick CI on 94dd318: 1379 passed, 0 failed, 13 xfail.
- Full `cargo xtask ci` on c378c75: green (kiln 1379 passed / 13 xfail;
  clang self-check 1365 passed / 3 xfail / 24 skipped; ir_vs_clang
  180/180 exact = 90 programs x 2 triples).
- Done; handed back.
