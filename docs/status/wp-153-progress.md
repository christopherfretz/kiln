# WP-153 progress: goto cleanups, jump-scope diagnostics, noreturn

Branch `wp/153-goto-cleanups-noreturn`, worktree `kiln/.worktrees/wp-153`.

## Done
- **Goto cleanups.** A local whose default constructor is trivial now
  pushes its destructor cleanup (clang `EmitAutoVarCleanups`), so a `goto`
  out of its scope (and a plain scope exit or an exception) destroys it.
  - Code: `kiln_codegen/src/expr.rs:2771` (auto-var cleanups).
  - Tests: `tests/codegen/goto_cleanups.cpp`,
    `tests/conformance/stmt.goto/goto-cleanups-run.cpp`.
- **Jump-scope diagnostics.** A port of clang's `JumpScopeChecker`
  (`kiln_sema/src/jump_scope.rs`, run from `kiln_sema/src/stmt.rs:793`).
  It covers goto, switch/case and asm goto, entering a variable's scope,
  try/catch, constexpr/consteval `if`, statement expressions, and
  lifetime-extended temporaries (spec/sema.md §7.4).
  - Test: `tests/conformance/stmt.goto/jump-scope-diag.cpp`.
- **`[[noreturn]]`.**
  - Codegen: the `noreturn` attribute goes on declarations and
    definitions, and on direct call sites. Each such call is followed by
    `unreachable`, as in clang (`kiln_codegen/src/expr.rs:1845`
    `noreturn_call`, `kiln_codegen/src/class.rs` `noreturn_extra`).
  - Sema: fall-off analysis now looks at every redeclaration and at member
    calls (`kiln_sema/src/stmt.rs`).
  - Tests: `tests/codegen/noreturn_calls.cpp`,
    `dcl.attr.noreturn/redecl-and-member-calls.cpp`.
- **`__builtin_isfpclass` and `__builtin_expect_with_probability`.**
  - Sema checks: `kiln_sema/src/builtins.rs:465` (isfpclass) and `:429`
    (probability).
  - Constant evaluation: Oven `bi::MASK`.
  - Codegen: `llvm.is.fpclass`.
  - Tests: `expr.const/builtins-isfpclass{,-errors}.cpp`,
    `tests/codegen/isfpclass.cpp`.
- **Source location in default member initializers.**
  `__builtin_LINE`/`source_location::current()` in a default member
  initializer reports the location of its use:
  - a constructor's name, or the class name for an implicit constructor;
  - the closing `}` of an aggregate initialization.
  - Code: `kiln_sema/src/builtins.rs:1384` and `class/special/define.rs:635`.
    The Oven and codegen read the value through `enter_default_arg`; the
    codegen prepass folds it per use (`kiln_codegen/src/fold.rs:389`).
  - Test: `expr.const/builtins-srcloc-member-init-run.cpp`.
- **Labels, gotos and asm goto labels in templates** are substituted
  (`kiln_sema/src/templ/subst/stmts.rs:237`).
  - Tests: `stmt.goto/goto-in-template-run.cpp`,
    `stmt.goto/jump-scope-template.cpp`.

## Left
- **`-Wassume`.** It needs clang's possible-effects `HasSideEffects`
  (calls, volatile reads). kiln's `side_effects.rs` only has the
  definite-effects flavour.
- **`CXXTryStmt` substitution in templates.** It needs the handler's
  exception declaration substituted through `act_on_exception_decl`
  (DeclSpec/Declarator).
- **asm goto labels in the AST dump.**
- **`CXXParenListInitExpr` codegen** (`Agg a(3)`), which predates this WP.
