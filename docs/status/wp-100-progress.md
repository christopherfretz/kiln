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

## Step 3 — codegen EH (in progress)
- Done: `kiln_codegen/src/eh.rs` (EH scope stack use, `getInvokeDest`,
  `EmitLandingPad`, `getEHDispatchBlock`, `eh.resume`, terminate
  landing pad / handler, `__clang_call_terminate`, personality),
  call-or-invoke in `call_vals`/operator new/bad_cast, terminate scope
  of noexcept functions, `FinishFunction` EH block order,
  `TryMarkNoThrow` counting invokes and `resume`. Cleanup kinds
  `FreeException` / `EndCatch`. `CurrentCleanupScopeDepth`.
  Dtor cleanups on unwind match clang (scratch t1/t3 exact, both triples).
- Next: throw expression, try/catch (dispatch, handlers, catch params),
  function-try-blocks, ctor partial-construction EH cleanups, `new`
  dealloc cleanup, IR-diff corpus `tests/codegen/eh_*.cpp`.

## Step 4 — wrap-up (todo)
- run tests flip, spec §7b / sema / ast / parse notes, full CI.
