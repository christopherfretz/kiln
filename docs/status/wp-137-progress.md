# WP-137 progress: consteval follow-ups (from WP-133)

Branch `wp/137-consteval-followups`, worktree `kiln/.worktrees/wp-137`.

## Done
1. **Default uses re-checked** (spec/sema.md §5.10 "Default uses").
   - Each use of a default argument or default member initializer whose
     definition holds an unvalued immediate invocation is recorded
     (`note_default_use`, `kiln_sema/src/consteval.rs:201`) and re-evaluated
     at the end of the using full-expression (`check_default_use`,
     `consteval.rs:287`), with clang's `in the default initializer of`
     note.
   - Use sites: `expr.rs:2972` (call), `class/special/define.rs:1170`
     (ctor default args), `init.rs:475` (aggregate), `define.rs:586`
     (ctor member init, escalating defaulted/implicit ctors with the
     `immediate constructor because the default initializer` note),
     `templ/subst/shared.rs:89` and `templ/subst/exprs.rs:974`
     (instantiation re-check).
   - `D d;` default-init now finishes its immediate invocations
     (`class/special/uses.rs:176`).
   - Tests: `dcl.constexpr/consteval-default-{arg-use,init-use,init-escalation,use-run}.cpp`.
2. **Shared template expressions odr-use** their functions at each
   instantiation (`templ/subst/shared.rs`; test
   `temp.inst/shared-expr-odr-use.cpp`).
3. **Lambda conversion** of an escalated lambda is immediate; the Oven
   rejects a constant result pointing to an immediate function
   (`kiln_oven/src/value.rs`, `lambda/mod.rs`).
4. **`&C::m`** of a consteval member is reported at the qualifier
   (`class/member_expr.rs`; test `dcl.constexpr/consteval-address-qualified.cpp`).
5. **Vtable use**: ctor/dtor/key-function definitions mark the vtable's
   virtual functions referenced (`class/virtual/used.rs`; tests
   `basic.def.odr/vtable-use-*.cpp`; spec/sema-class.md §3.5).
6. **Implicit `operator=`** exception spec resolved before its
   DeclRefExpr type is built (`overload/oper.rs` `resolve_exc_for_type`).
7. **Oversized-constant store** codegen IR-diff case.

## Not done / follow-ups
- Implicit ctor escalation through a member whose default construction
  (no NSDMI) is a failed consteval ctor call; implicit mem-init immediate
  invocations in implicit ctor definitions.
- clang 19's duplicate report at class template NSDMI instantiation and
  inside `sizeof` (intentionally not replicated).
- Source-location builtins in an NSDMI used by aggregate init take the
  definition location.
- Generic lambda conversion escalation is not tracked per specialization.
- Explicit instantiation declaration vtable suppression.
- `[[no_unique_address]]` member panic in `kiln_abi/src/irtype.rs`; base
  initialized from a call stores the full object size; consteval base
  init elides clang's move construction.
- Global `operator new`/`delete` not implicitly declared for lookup.
- Pre-existing `-Wunused-value` on `X(1);`.
