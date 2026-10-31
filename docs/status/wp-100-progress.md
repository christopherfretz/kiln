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
- Done: throw expression (`__cxa_allocate_exception` / `__cxa_throw`,
  FreeException deactivation), try/catch (`try_catch.rs`: catch scopes,
  dispatch via `llvm.eh.typeid.for`, handler blocks after uses,
  `InitCatchParam` for ref/pointer/scalar/trivial/non-trivial class,
  EndCatch cleanup, rethrow); fall-off `llvm.trap` without call-site
  attributes (clang `EmitTrapCall`). Scratch tc1 exact on both triples.
- Done: ctor/dtor function-try-blocks (try entered before the body scope,
  implicit `__cxa_rethrow`, abstract complete dtor trap, complete dtor
  with try = base + vbase cleanups); partial-construction EH cleanups
  (base dtors, member dtors, delegating ctor, memcpyizer); partial array
  cleanup in array construction (`pushRegularPartialArrayCleanup`);
  new-expression delete cleanup (`CallDeleteDuringNew`, saved values in
  conditional branches, dominator placeholder + deactivation), class
  operator new result type. Sema: brace elision only into aggregates
  ([dcl.init.aggr]/16; `init.rs`, one condition).
- Found (not EH): `new G[n]` for a class with a non-trivial dtor has no
  array cookie and a wrong nested loop (silent miscompile); aligned new
  picks the sized aligned delete (clang the unsized one).
- Next: CI quick, corpus `tests/codegen/eh_*.cpp`.

## Step 4 — wrap-up (todo)
- run tests flip, spec §7b / sema / ast / parse notes, full CI.
